# micro:bit Sensor Games

Interactive CLI games controlled by BBC micro:bit accelerometer sensors using Rust and MicroPython.

## Features

- **Real-time sensor reading** from micro:bit via USB serial
- **Three interactive applications**:
  1. Sensor data reader - Monitor accelerometer, temperature, and light levels
  2. Rhythm Game - Shake to the beat
  3. Cannon Defense - Tilt-to-shoot arcade game

## Hardware Requirements

- BBC micro:bit (v1 or v2)
- USB cable
- Linux system (tested on Ubuntu/Debian)

## Setup

### 1. Configure USB Permissions

Run the setup script to configure udev rules and user groups:

```bash
sudo ./setup.sh
```

**Important**: Log out and log back in after running setup for group changes to take effect.

### 2. Flash micro:bit with MicroPython Firmware

The sensor reader firmware is already included:

```bash
# Using uv (recommended)
uv venv
uv pip install uflash
.venv/bin/uflash sensor_reader.py /media/$USER/MICROBIT/
```

### 3. Build Rust Applications

```bash
cargo build --release
```

## Applications

### 1. Sensor Reader

Monitor real-time sensor data from your micro:bit:

```bash
./target/release/microbit_reader
```

**Output:**
- Accelerometer (X, Y, Z in mg and g)
- Temperature (°C)
- Light level (0-255)
- Timestamp

### 2. Rhythm Game

A musical rhythm game where you shake the micro:bit to hit beats:

```bash
./target/release/rhythm_game
```

**How to Play:**
- Diamond beats (◆) fall down the screen
- Shake the micro:bit when they hit the green zone
- Build combos for higher scores
- 30-second song with dynamic patterns

**Scoring:**
- Perfect hit: 100 points × combo
- Good hit: 50 points × combo
- Miss: Breaks combo

### 3. Cannon Defense

A tilt-controlled shooting game where you defend against incoming enemies:

```bash
./target/release/cannon_defense
```

**How to Play:**
- You are the cannon (⊕) in the center
- Enemies (●) approach from all sides
- **Tilt** the micro:bit to aim
- Bullets fire automatically when tilted
- Survive waves of increasing difficulty

**Game Mechanics:**
- Health: 100 HP (enemies reaching center = -10 HP)
- Waves: Complete 10 kills per wave to advance
- Difficulty: Enemy speed and spawn rate increase each wave
- Scoring: 10 × wave number points per kill

**Controls:**
- Tilt forward/back/left/right = Aim & shoot
- Flat (no tilt) = Stop shooting
- Press 'q' = Quit

## Technical Details

### Sensor Data Format

The micro:bit outputs CSV data over serial at 115200 baud:

```
timestamp,accel_x,accel_y,accel_z,temperature,light_level
0,120,-180,960,24,0
1,124,-176,968,24,0
```

- **Accelerometer**: ±2g range, values in milligravity (mg)
- **Temperature**: Degrees Celsius
- **Light Level**: 0-255 (LED matrix as sensor)
- **Sample Rate**: 2 Hz (500ms intervals)

### Architecture

- **MicroPython Firmware** (`sensor_reader.py`): Runs on micro:bit, reads sensors, outputs to serial
- **Rust Applications**: Read serial data, process input, render terminal UI
- **Dependencies**:
  - `serialport` - USB serial communication
  - `crossterm` - Terminal manipulation and rendering
  - `anyhow` - Error handling

## Troubleshooting

### Permission Denied on /dev/ttyACM0

Make sure you've run `setup.sh` and logged out/in. Check your groups:

```bash
groups
```

You should see both `dialout` and `plugdev`.

Temporary workaround:

```bash
sg dialout -c "./target/release/microbit_reader"
```

### micro:bit Not Detected

Check USB connection:

```bash
lsusb | grep 0d28:0204
ls -la /dev/ttyACM0
```

### Serial Port Busy

Kill any processes holding the port:

```bash
lsof /dev/ttyACM0
# Then kill the process
```

## Project Structure

```
m-bit/
├── Cargo.toml              # Rust project configuration
├── sensor_reader.py        # MicroPython firmware for micro:bit
├── setup.sh               # Linux USB permissions setup
├── src/
│   ├── main.rs            # Sensor data reader
│   ├── rhythm_game.rs     # Rhythm game
│   └── cannon_defense.rs  # Cannon defense game
└── README.md
```

## License

MIT

## Acknowledgments

Built with [Claude Code](https://claude.com/claude-code) - AI-powered coding assistant
