use anyhow::{Context, Result};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use serialport::SerialPort;
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
    accel_z: i16,
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
            accel_z: parts[3].parse().context("Invalid accel_z")?,
        })
    }

    fn magnitude(&self) -> f32 {
        let x = self.accel_x as f32;
        let y = self.accel_y as f32;
        let z = self.accel_z as f32;
        (x * x + y * y + z * z).sqrt()
    }
}

#[derive(Debug, Clone)]
struct Beat {
    position: f32,
    lane: usize,
    hit: bool,
}

#[derive(Debug, PartialEq)]
enum HitQuality {
    Perfect,
    Good,
    Miss,
}

struct Game {
    beats: Vec<Beat>,
    score: u32,
    combo: u32,
    max_combo: u32,
    perfect_hits: u32,
    good_hits: u32,
    misses: u32,
    shake_threshold: f32,
    last_magnitude: f32,
    game_time: f32,
    beat_speed: f32,
    hit_zone: f32,
    width: u16,
    height: u16,
}

impl Game {
    fn new(width: u16, height: u16) -> Self {
        let mut game = Game {
            beats: Vec::new(),
            score: 0,
            combo: 0,
            max_combo: 0,
            perfect_hits: 0,
            good_hits: 0,
            misses: 0,
            shake_threshold: 300.0,
            last_magnitude: 0.0,
            game_time: 0.0,
            beat_speed: 8.0,
            hit_zone: height as f32 - 3.0,
            width,
            height,
        };
        game.spawn_beat_pattern();
        game
    }

    fn spawn_beat_pattern(&mut self) {
        let patterns = vec![
            vec![0.0, 1.0, 2.0, 3.0],
            vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5],
            vec![0.0, 1.0, 1.5, 2.5, 3.0, 4.0],
            vec![0.0, 0.75, 1.5, 2.25, 3.0],
        ];

        let pattern = &patterns[self.game_time as usize % patterns.len()];
        let base_offset = self.game_time;

        for &offset in pattern {
            self.beats.push(Beat {
                position: -(offset * self.beat_speed),
                lane: 0,
                hit: false,
            });
        }
    }

    fn update(&mut self, delta_time: f32, sensor_data: Option<SensorData>) -> bool {
        self.game_time += delta_time;

        if self.game_time.fract() < delta_time && self.game_time > 1.0 {
            self.spawn_beat_pattern();
        }

        let mut shake_detected = false;
        if let Some(ref data) = sensor_data {
            let magnitude = data.magnitude();
            let delta_mag = (magnitude - self.last_magnitude).abs();

            if delta_mag > self.shake_threshold {
                shake_detected = true;
            }
            self.last_magnitude = magnitude;
        }

        for beat in &mut self.beats {
            beat.position += self.beat_speed * delta_time;
        }

        if shake_detected {
            self.process_shake();
        }

        self.beats.retain(|beat| {
            if beat.position > self.height as f32 + 2.0 && !beat.hit {
                false
            } else {
                true
            }
        });

        let mut missed = 0;
        for beat in &mut self.beats {
            if !beat.hit && beat.position > self.hit_zone + 3.0 {
                beat.hit = true;
                missed += 1;
            }
        }

        if missed > 0 {
            self.register_miss(missed);
        }

        self.game_time < 30.0
    }

    fn process_shake(&mut self) {
        let mut best_hit: Option<(usize, f32)> = None;

        for (i, beat) in self.beats.iter().enumerate() {
            if beat.hit {
                continue;
            }

            let distance = (beat.position - self.hit_zone).abs();
            if distance < 5.0 {
                if let Some((_, best_dist)) = best_hit {
                    if distance < best_dist {
                        best_hit = Some((i, distance));
                    }
                } else {
                    best_hit = Some((i, distance));
                }
            }
        }

        if let Some((idx, distance)) = best_hit {
            self.beats[idx].hit = true;

            let quality = if distance < 1.0 {
                HitQuality::Perfect
            } else if distance < 2.5 {
                HitQuality::Good
            } else {
                HitQuality::Miss
            };

            self.register_hit(quality);
        }
    }

    fn register_hit(&mut self, quality: HitQuality) {
        match quality {
            HitQuality::Perfect => {
                self.score += 100 * (1 + self.combo / 10);
                self.combo += 1;
                self.perfect_hits += 1;
            }
            HitQuality::Good => {
                self.score += 50 * (1 + self.combo / 10);
                self.combo += 1;
                self.good_hits += 1;
            }
            HitQuality::Miss => {
                self.combo = 0;
                self.misses += 1;
            }
        }

        if self.combo > self.max_combo {
            self.max_combo = self.combo;
        }
    }

    fn register_miss(&mut self, count: u32) {
        self.misses += count;
        self.combo = 0;
    }

    fn render(&self) -> Result<()> {
        let mut stdout = stdout();

        execute!(stdout, Clear(ClearType::All))?;

        execute!(
            stdout,
            MoveTo(2, 1),
            SetForegroundColor(Color::Cyan),
            Print("♪ MICRO:BIT RHYTHM GAME ♪"),
            ResetColor
        )?;

        execute!(
            stdout,
            MoveTo(2, 2),
            Print(format!("Score: {}  Combo: x{}", self.score, self.combo))
        )?;

        let time_left = (30.0 - self.game_time).max(0.0);
        execute!(
            stdout,
            MoveTo(self.width - 15, 2),
            Print(format!("Time: {:.1}s", time_left))
        )?;

        let lane_x = self.width / 2;

        for y in 0..self.height {
            execute!(stdout, MoveTo(lane_x, y), Print("│"))?;
        }

        let hit_y = self.hit_zone as u16;
        execute!(
            stdout,
            MoveTo(lane_x - 3, hit_y),
            SetForegroundColor(Color::Green),
            Print("══╪══"),
            ResetColor
        )?;

        for beat in &self.beats {
            if beat.hit {
                continue;
            }

            let y = beat.position as u16;
            if y < self.height {
                let distance = (beat.position - self.hit_zone).abs();
                let (symbol, color) = if distance < 1.0 {
                    ("◆", Color::Yellow)
                } else if distance < 2.5 {
                    ("◆", Color::Cyan)
                } else {
                    ("◆", Color::White)
                };

                execute!(
                    stdout,
                    MoveTo(lane_x, y),
                    SetForegroundColor(color),
                    Print(symbol),
                    ResetColor
                )?;
            }
        }

        execute!(
            stdout,
            MoveTo(2, self.height - 1),
            SetForegroundColor(Color::DarkGrey),
            Print("SHAKE the micro:bit when ◆ hits the green zone!"),
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
            MoveTo(center_x - 10, center_y - 6),
            SetForegroundColor(Color::Cyan),
            Print("═══════════════════════"),
            ResetColor
        )?;

        execute!(
            stdout,
            MoveTo(center_x - 5, center_y - 5),
            SetForegroundColor(Color::Yellow),
            Print("GAME OVER!"),
            ResetColor
        )?;

        execute!(
            stdout,
            MoveTo(center_x - 10, center_y - 4),
            SetForegroundColor(Color::Cyan),
            Print("═══════════════════════"),
            ResetColor
        )?;

        execute!(stdout, MoveTo(center_x - 10, center_y - 2), Print(format!("Final Score: {}", self.score)))?;
        execute!(stdout, MoveTo(center_x - 10, center_y - 1), Print(format!("Max Combo: x{}", self.max_combo)))?;
        execute!(stdout, MoveTo(center_x - 10, center_y + 1), SetForegroundColor(Color::Green), Print(format!("Perfect: {}", self.perfect_hits)), ResetColor)?;
        execute!(stdout, MoveTo(center_x - 10, center_y + 2), SetForegroundColor(Color::Cyan), Print(format!("Good: {}", self.good_hits)), ResetColor)?;
        execute!(stdout, MoveTo(center_x - 10, center_y + 3), SetForegroundColor(Color::Red), Print(format!("Miss: {}", self.misses)), ResetColor)?;

        let total_notes = self.perfect_hits + self.good_hits + self.misses;
        let accuracy = if total_notes > 0 {
            ((self.perfect_hits + self.good_hits) as f32 / total_notes as f32) * 100.0
        } else {
            0.0
        };

        execute!(
            stdout,
            MoveTo(center_x - 10, center_y + 5),
            SetForegroundColor(Color::Yellow),
            Print(format!("Accuracy: {:.1}%", accuracy)),
            ResetColor
        )?;

        execute!(
            stdout,
            MoveTo(center_x - 15, self.height - 2),
            SetForegroundColor(Color::DarkGrey),
            Print("Press any key to exit..."),
            ResetColor
        )?;

        stdout.flush()?;
        Ok(())
    }
}

fn sensor_reader_thread(
    sensor_data: Arc<Mutex<Option<SensorData>>>,
) -> Result<()> {
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

        let current_sensor = sensor_data.lock().unwrap().clone();

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
