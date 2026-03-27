use anyhow::{Context, Result};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{stdout, BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const SERIAL_PORT: &str = "/dev/ttyACM0";
const BAUD_RATE: u32 = 115200;

#[derive(Debug, Clone)]
struct SensorData {
    accel_x: i16,
    accel_y: i16,
}

impl SensorData {
    fn from_csv(line: &str) -> Result<Self> {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 6 {
            anyhow::bail!("Invalid CSV format");
        }

        Ok(SensorData {
            accel_x: parts[1].parse().context("Invalid accel_x")?,
            accel_y: parts[2].parse().context("Invalid accel_y")?,
        })
    }

    fn get_angle(&self) -> f32 {
        let x = self.accel_x as f32;
        let y = -self.accel_y as f32;
        y.atan2(x)
    }

    fn is_tilted(&self) -> bool {
        let x = self.accel_x.abs();
        let y = self.accel_y.abs();
        x > 150 || y > 150
    }
}

#[derive(Debug, Clone)]
struct Vec2 {
    x: f32,
    y: f32,
}

impl Vec2 {
    fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }

    fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    fn normalize(&self) -> Vec2 {
        let len = self.length();
        if len > 0.0 {
            Vec2::new(self.x / len, self.y / len)
        } else {
            Vec2::new(0.0, 0.0)
        }
    }

    fn distance_to(&self, other: &Vec2) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

#[derive(Debug)]
struct Bullet {
    pos: Vec2,
    vel: Vec2,
    lifetime: f32,
}

impl Bullet {
    fn new(pos: Vec2, angle: f32, speed: f32) -> Self {
        Bullet {
            pos,
            vel: Vec2::new(angle.cos() * speed, angle.sin() * speed),
            lifetime: 3.0,
        }
    }

    fn update(&mut self, dt: f32) -> bool {
        self.pos.x += self.vel.x * dt;
        self.pos.y += self.vel.y * dt;
        self.lifetime -= dt;
        self.lifetime > 0.0
    }
}

#[derive(Debug)]
struct Enemy {
    pos: Vec2,
    vel: Vec2,
    health: i32,
    spawn_time: f32,
}

impl Enemy {
    fn new(pos: Vec2, target: Vec2, speed: f32, spawn_time: f32) -> Self {
        let dir = Vec2::new(target.x - pos.x, target.y - pos.y).normalize();
        Enemy {
            pos,
            vel: Vec2::new(dir.x * speed, dir.y * speed),
            health: 1,
            spawn_time,
        }
    }

    fn update(&mut self, dt: f32) {
        self.pos.x += self.vel.x * dt;
        self.pos.y += self.vel.y * dt;
    }

    fn is_alive(&self) -> bool {
        self.health > 0
    }
}

struct Game {
    center: Vec2,
    bullets: Vec<Bullet>,
    enemies: Vec<Enemy>,
    score: u32,
    health: i32,
    wave: u32,
    game_time: f32,
    last_shot: f32,
    fire_rate: f32,
    aim_angle: f32,
    enemies_killed: u32,
    spawn_timer: f32,
    spawn_interval: f32,
    width: u16,
    height: u16,
}

impl Game {
    fn new(width: u16, height: u16) -> Self {
        Game {
            center: Vec2::new(width as f32 / 2.0, height as f32 / 2.0),
            bullets: Vec::new(),
            enemies: Vec::new(),
            score: 0,
            health: 100,
            wave: 1,
            game_time: 0.0,
            last_shot: 0.0,
            fire_rate: 0.15,
            aim_angle: 0.0,
            enemies_killed: 0,
            spawn_timer: 0.0,
            spawn_interval: 1.5,
            width,
            height,
        }
    }

    fn spawn_enemy(&mut self) {
        let margin = 2.0;

        let spawn_pos = if rand_bool(self.game_time) {
            if rand_bool(self.game_time * 2.0) {
                Vec2::new(margin, (self.game_time * 73.0) % (self.height as f32 - margin * 2.0) + margin)
            } else {
                Vec2::new(self.width as f32 - margin, (self.game_time * 97.0) % (self.height as f32 - margin * 2.0) + margin)
            }
        } else if rand_bool(self.game_time * 3.0) {
            Vec2::new((self.game_time * 113.0) % (self.width as f32 - margin * 2.0) + margin, margin)
        } else {
            Vec2::new((self.game_time * 127.0) % (self.width as f32 - margin * 2.0) + margin, self.height as f32 - margin)
        };

        let base_speed = 5.0 + (self.wave as f32 * 0.5);
        let enemy = Enemy::new(spawn_pos, self.center.clone(), base_speed, self.game_time);
        self.enemies.push(enemy);
    }

    fn update(&mut self, dt: f32, sensor_data: Option<SensorData>) -> bool {
        if self.health <= 0 {
            return false;
        }

        self.game_time += dt;
        self.spawn_timer += dt;

        if let Some(ref data) = sensor_data {
            self.aim_angle = data.get_angle();

            if data.is_tilted() && (self.game_time - self.last_shot) >= self.fire_rate {
                self.fire_bullet();
                self.last_shot = self.game_time;
            }
        }

        let current_interval = self.spawn_interval / (1.0 + self.wave as f32 * 0.1);
        if self.spawn_timer >= current_interval {
            self.spawn_enemy();
            self.spawn_timer = 0.0;
        }

        self.bullets.retain_mut(|bullet| bullet.update(dt));

        for enemy in &mut self.enemies {
            enemy.update(dt);

            if enemy.pos.distance_to(&self.center) < 2.0 {
                if enemy.is_alive() {
                    self.health -= 10;
                    enemy.health = 0;
                }
            }
        }

        for bullet in &mut self.bullets {
            for enemy in &mut self.enemies {
                if enemy.is_alive() && bullet.pos.distance_to(&enemy.pos) < 1.5 {
                    enemy.health -= 1;
                    if !enemy.is_alive() {
                        self.score += 10 * self.wave;
                        self.enemies_killed += 1;
                    }
                    bullet.lifetime = 0.0;
                }
            }
        }

        self.enemies.retain(|e| e.is_alive() &&
            e.pos.x > 0.0 && e.pos.x < self.width as f32 &&
            e.pos.y > 0.0 && e.pos.y < self.height as f32);

        if self.enemies_killed >= self.wave * 10 {
            self.wave += 1;
            self.enemies_killed = 0;
        }

        true
    }

    fn fire_bullet(&mut self) {
        let bullet = Bullet::new(
            self.center.clone(),
            self.aim_angle,
            30.0,
        );
        self.bullets.push(bullet);
    }

    fn render(&self) -> Result<()> {
        let mut stdout = stdout();
        execute!(stdout, Clear(ClearType::All))?;

        execute!(
            stdout,
            MoveTo(2, 1),
            SetForegroundColor(Color::Yellow),
            Print("⚔ CANNON DEFENSE ⚔"),
            ResetColor
        )?;

        execute!(
            stdout,
            MoveTo(2, 2),
            Print(format!("Score: {}  Wave: {}  Kills: {}/{}",
                self.score, self.wave, self.enemies_killed, self.wave * 10))
        )?;

        let health_bar_width = 20;
        let filled = (self.health as f32 / 100.0 * health_bar_width as f32) as usize;
        let health_color = if self.health > 60 {
            Color::Green
        } else if self.health > 30 {
            Color::Yellow
        } else {
            Color::Red
        };

        execute!(
            stdout,
            MoveTo(self.width.saturating_sub(25), 2),
            Print("HP: "),
            SetForegroundColor(health_color),
            Print("█".repeat(filled)),
            SetForegroundColor(Color::DarkGrey),
            Print("░".repeat(health_bar_width - filled)),
            ResetColor,
            Print(format!(" {}", self.health))
        )?;

        let cx = self.center.x as u16;
        let cy = self.center.y as u16;

        let aim_len = 3.0;
        let aim_x = (self.center.x + self.aim_angle.cos() * aim_len) as u16;
        let aim_y = (self.center.y + self.aim_angle.sin() * aim_len) as u16;

        if aim_x < self.width && aim_y < self.height {
            execute!(
                stdout,
                MoveTo(aim_x, aim_y),
                SetForegroundColor(Color::Cyan),
                Print("→"),
                ResetColor
            )?;
        }

        execute!(
            stdout,
            MoveTo(cx, cy),
            SetForegroundColor(Color::Yellow),
            Print("⊕"),
            ResetColor
        )?;

        for enemy in &self.enemies {
            let ex = enemy.pos.x as u16;
            let ey = enemy.pos.y as u16;

            if ex > 0 && ex < self.width && ey > 0 && ey < self.height {
                let age = self.game_time - enemy.spawn_time;
                let (symbol, color) = if age < 0.3 {
                    ("◉", Color::DarkRed)
                } else {
                    ("●", Color::Red)
                };

                execute!(
                    stdout,
                    MoveTo(ex, ey),
                    SetForegroundColor(color),
                    Print(symbol),
                    ResetColor
                )?;
            }
        }

        for bullet in &self.bullets {
            let bx = bullet.pos.x as u16;
            let by = bullet.pos.y as u16;

            if bx > 0 && bx < self.width && by > 0 && by < self.height {
                execute!(
                    stdout,
                    MoveTo(bx, by),
                    SetForegroundColor(Color::Cyan),
                    Print("•"),
                    ResetColor
                )?;
            }
        }

        execute!(
            stdout,
            MoveTo(2, self.height - 1),
            SetForegroundColor(Color::DarkGrey),
            Print("TILT the micro:bit to aim and shoot! Keep enemies away from center!"),
            ResetColor
        )?;

        execute!(
            stdout,
            MoveTo(2, self.height),
            SetForegroundColor(Color::DarkGrey),
            Print("Press 'q' to quit"),
            ResetColor
        )?;

        stdout.flush()?;
        Ok(())
    }

    fn show_game_over(&self) -> Result<()> {
        let mut stdout = stdout();
        execute!(stdout, Clear(ClearType::All))?;

        let center_x = self.width / 2;
        let center_y = self.height / 2;

        execute!(
            stdout,
            MoveTo(center_x.saturating_sub(10), center_y.saturating_sub(6)),
            SetForegroundColor(Color::Red),
            Print("═══════════════════════"),
            ResetColor
        )?;

        execute!(
            stdout,
            MoveTo(center_x.saturating_sub(5), center_y.saturating_sub(5)),
            SetForegroundColor(Color::Red),
            Print("GAME OVER!"),
            ResetColor
        )?;

        execute!(
            stdout,
            MoveTo(center_x.saturating_sub(10), center_y.saturating_sub(4)),
            SetForegroundColor(Color::Red),
            Print("═══════════════════════"),
            ResetColor
        )?;

        execute!(
            stdout,
            MoveTo(center_x.saturating_sub(10), center_y.saturating_sub(2)),
            SetForegroundColor(Color::Yellow),
            Print(format!("Final Score: {}", self.score)),
            ResetColor
        )?;

        execute!(
            stdout,
            MoveTo(center_x.saturating_sub(10), center_y.saturating_sub(1)),
            Print(format!("Waves Survived: {}", self.wave))
        )?;

        execute!(
            stdout,
            MoveTo(center_x.saturating_sub(10), center_y),
            SetForegroundColor(Color::Cyan),
            Print(format!("Enemies Destroyed: {}", self.score / 10)),
            ResetColor
        )?;

        execute!(
            stdout,
            MoveTo(center_x.saturating_sub(10), center_y + 2),
            Print(format!("Survival Time: {:.1}s", self.game_time))
        )?;

        execute!(
            stdout,
            MoveTo(center_x.saturating_sub(15), self.height.saturating_sub(2)),
            SetForegroundColor(Color::DarkGrey),
            Print("Press any key to exit..."),
            ResetColor
        )?;

        stdout.flush()?;
        Ok(())
    }
}

fn rand_bool(seed: f32) -> bool {
    ((seed * 1234.567).sin() * 10000.0).fract() > 0.5
}

fn sensor_reader_thread(sensor_data: Arc<Mutex<Option<SensorData>>>) -> Result<()> {
    let port = serialport::new(SERIAL_PORT, BAUD_RATE)
        .timeout(Duration::from_secs(1))
        .open()
        .context(format!("Failed to open {}", SERIAL_PORT))?;

    let mut reader = BufReader::new(port);
    let mut line = String::new();
    let mut skip_header = true;

    loop {
        line.clear();
        if reader.read_line(&mut line).is_err() {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if skip_header {
            if trimmed == "---" {
                skip_header = false;
                continue;
            }
            if SensorData::from_csv(trimmed).is_ok() {
                skip_header = false;
            } else {
                continue;
            }
        }

        if let Ok(data) = SensorData::from_csv(trimmed) {
            if let Ok(mut sensor) = sensor_data.lock() {
                *sensor = Some(data);
            }
        }
    }
}

fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(stdout(), Show, LeaveAlternateScreen);
        original_hook(info);
    }));
}

fn main() -> Result<()> {
    let sensor_data = Arc::new(Mutex::new(None));
    let sensor_data_clone = Arc::clone(&sensor_data);

    thread::spawn(move || {
        if let Err(e) = sensor_reader_thread(sensor_data_clone) {
            eprintln!("Sensor reader error: {}", e);
        }
    });

    thread::sleep(Duration::from_secs(1));

    terminal::enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, Hide)?;
    install_panic_hook();

    let (width, height) = terminal::size()?;
    let mut game = Game::new(width, height);

    let mut last_update = Instant::now();
    let target_fps = 60;
    let frame_duration = Duration::from_millis(1000 / target_fps);

    let result = 'game_loop: loop {
        let now = Instant::now();
        let delta_time = (now - last_update).as_secs_f32();
        last_update = now;

        if event::poll(Duration::from_millis(1))? {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                if code == KeyCode::Char('q') {
                    break 'game_loop Ok(());
                }
            }
        }

        let current_sensor = sensor_data.lock().ok().and_then(|guard| guard.clone());

        if !game.update(delta_time, current_sensor) {
            game.show_game_over()?;

            loop {
                if event::poll(Duration::from_secs(10))? {
                    event::read()?;
                    break;
                }
            }
            break 'game_loop Ok(());
        }

        game.render()?;

        let elapsed = now.elapsed();
        if elapsed < frame_duration {
            thread::sleep(frame_duration - elapsed);
        }
    };

    execute!(stdout, Show, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    result
}
