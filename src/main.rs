use anyhow::{Context, Result};
use serialport::SerialPort;
use std::io::{BufRead, BufReader};
use std::time::Duration;

const SERIAL_PORT: &str = "/dev/ttyACM0";
const BAUD_RATE: u32 = 115200;

#[derive(Debug)]
struct SensorData {
    timestamp: u32,
    accel_x: i16,
    accel_y: i16,
    accel_z: i16,
    temperature: i8,
    light_level: u8,
}

impl SensorData {
    fn from_csv(line: &str) -> Result<Self> {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 6 {
            anyhow::bail!("Invalid CSV format: expected 6 fields, got {}", parts.len());
        }

        Ok(SensorData {
            timestamp: parts[0].parse().context("Invalid timestamp")?,
            accel_x: parts[1].parse().context("Invalid accel_x")?,
            accel_y: parts[2].parse().context("Invalid accel_y")?,
            accel_z: parts[3].parse().context("Invalid accel_z")?,
            temperature: parts[4].parse().context("Invalid temperature")?,
            light_level: parts[5].parse().context("Invalid light_level")?,
        })
    }

    fn display(&self) {
        println!("─────────────────────────────────────────────────────");
        println!("Timestamp: {} (x500ms = {:.1}s)", self.timestamp, self.timestamp as f32 * 0.5);
        println!("Accelerometer:");
        println!("  X: {:>6} mg ({:>6.2} g)", self.accel_x, self.accel_x as f32 / 1000.0);
        println!("  Y: {:>6} mg ({:>6.2} g)", self.accel_y, self.accel_y as f32 / 1000.0);
        println!("  Z: {:>6} mg ({:>6.2} g)", self.accel_z, self.accel_z as f32 / 1000.0);
        println!("Temperature: {}°C", self.temperature);
        println!("Light Level: {}/255 ({:.1}%)", self.light_level, (self.light_level as f32 / 255.0) * 100.0);
    }
}

fn open_serial_port() -> Result<Box<dyn SerialPort>> {
    let port = serialport::new(SERIAL_PORT, BAUD_RATE)
        .timeout(Duration::from_secs(10))
        .open()
        .context(format!("Failed to open serial port {}", SERIAL_PORT))?;

    println!("✓ Connected to micro:bit on {}", SERIAL_PORT);
    println!("✓ Baud rate: {}", BAUD_RATE);
    println!();

    Ok(port)
}

fn main() -> Result<()> {
    println!("═══════════════════════════════════════════════════════");
    println!("  BBC micro:bit Sensor Reader (Rust)");
    println!("═══════════════════════════════════════════════════════");
    println!();

    let port = open_serial_port()?;
    let mut reader = BufReader::new(port);

    println!("Reading sensor data... (Press Ctrl+C to stop)");
    println!();

    let mut line = String::new();
    let mut skip_header = true;

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)
            .context("Failed to read from serial port")?;

        if bytes_read == 0 {
            println!("⚠ Connection lost");
            break;
        }

        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        if skip_header {
            if trimmed == "---" {
                skip_header = false;
                println!("✓ Header received, starting data stream...");
                println!();
                continue;
            }

            if let Ok(data) = SensorData::from_csv(trimmed) {
                skip_header = false;
                println!("✓ Auto-detected valid sensor data, starting stream...");
                println!();
                data.display();
                continue;
            }

            println!("Header: {}", trimmed);
            continue;
        }

        match SensorData::from_csv(trimmed) {
            Ok(data) => data.display(),
            Err(e) => {
                println!("⚠ Parse error: {} (line: {:?})", e, trimmed);
            }
        }
    }

    Ok(())
}
