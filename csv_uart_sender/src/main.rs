use serialport::SerialPort;
use std::env;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_BAUD: u32 = 115_200;
const PACKET_HEADER_1: u8 = 0xAA;
const PACKET_HEADER_2: u8 = 0x55;
const PACKET_KIND_MOVE: u8 = 0;
const PACKET_KIND_DRAW: u8 = 1;
const MM_PER_PX_X: f32 = 0.10;
const MM_PER_PX_Y: f32 = 0.10;
const STEPS_PER_MM_X: f32 = 80.0;
const STEPS_PER_MM_Y: f32 = 80.0;

#[derive(Debug, Clone)]
struct Point {
    x: i32,
    y: i32,
    timestamp_ms: u128,
}

#[derive(Debug, Clone)]
struct Packet {
    kind: u8,
    steps_x: i16,
    steps_y: i16,
    dt_ms: u16,
}

#[derive(Debug, Clone)]
struct MotionSegment {
    kind: u8,
    dx_px: i32,
    dy_px: i32,
    dt_ms: u16,
    dx_mm: f32,
    dy_mm: f32,
    speed_mm_s: f32,
    steps_x: i16,
    steps_y: i16,
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::from_args()?;
    let csv_path = resolve_csv_path(config.csv_path)?;
    let points = load_points(&csv_path)?;
    if points.len() < 2 {
        return Err("CSV must contain at least 2 points".into());
    }

    let segments = build_segments(&points)?;
    println!("csv: {}", csv_path.display());
    println!("port: {}", config.port);
    println!("baud: {}", config.baud);
    println!("segments: {}", segments.len());
    println!(
        "calibration: mm_per_px=({:.4}, {:.4}), steps_per_mm=({:.2}, {:.2})",
        MM_PER_PX_X, MM_PER_PX_Y, STEPS_PER_MM_X, STEPS_PER_MM_Y
    );

    let mut port = open_port(&config.port, config.baud)?;
    for (idx, segment) in segments.iter().enumerate() {
        let bytes = encode_packet(segment);
        port.write_all(&bytes)?;
        port.flush()?;
        println!(
            "#{:04} kind={} px=({}, {}) mm=({:.3}, {:.3}) steps=({}, {}) dt={}ms speed={:.2}mm/s -> {:02X?}",
            idx,
            segment.kind,
            segment.dx_px,
            segment.dy_px,
            segment.dx_mm,
            segment.dy_mm,
            segment.steps_x,
            segment.steps_y,
            segment.dt_ms,
            segment.speed_mm_s,
            bytes
        );
    }

    Ok(())
}

struct Config {
    csv_path: Option<PathBuf>,
    port: String,
    baud: u32,
}

impl Config {
    fn from_args() -> Result<Self, Box<dyn Error>> {
        let mut csv_path = None;
        let mut port = None;
        let mut baud = DEFAULT_BAUD;

        let mut args = env::args().skip(1).peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--csv" => {
                    let value = args.next().ok_or("--csv requires a path")?;
                    csv_path = Some(PathBuf::from(value));
                }
                "--port" => {
                    port = Some(args.next().ok_or("--port requires a device path")?);
                }
                "--baud" => {
                    let value = args.next().ok_or("--baud requires a number")?;
                    baud = value.parse()?;
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                other if other.starts_with('-') => {
                    return Err(format!("unknown option: {other}").into());
                }
                value => {
                    if csv_path.is_none() {
                        csv_path = Some(PathBuf::from(value));
                    } else {
                        return Err(format!("unexpected positional argument: {value}").into());
                    }
                }
            }
        }

        let port = port.unwrap_or_else(|| "/dev/ttyUSB0".to_string());

        Ok(Self {
            csv_path,
            port,
            baud,
        })
    }
}

fn print_help() {
    println!(
        "Usage:\n  cargo run -- --port /dev/ttyUSB0 [--baud 115200] [--csv path/to/file.csv]\n\nIf --csv is omitted, the newest trajectory_*.csv is used from output/ or ../output/."
    );
}

fn resolve_csv_path(csv_path: Option<PathBuf>) -> Result<PathBuf, Box<dyn Error>> {
    if let Some(path) = csv_path {
        return Ok(path);
    }

    for dir in ["output", "../output"] {
        if let Some(path) = newest_csv_in(dir)? {
            return Ok(path);
        }
    }

    Err("no CSV provided and no trajectory_*.csv found in output/ or ../output/".into())
}

fn newest_csv_in(dir: impl AsRef<Path>) -> Result<Option<PathBuf>, Box<dyn Error>> {
    let dir = dir.as_ref();
    if !dir.exists() {
        return Ok(None);
    }

    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("csv") {
            continue;
        }

        let modified = entry
            .metadata()?
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        match &newest {
            Some((best_time, _)) if modified <= *best_time => {}
            _ => newest = Some((modified, path)),
        }
    }

    Ok(newest.map(|(_, path)| path))
}

fn load_points(path: &Path) -> Result<Vec<Point>, Box<dyn Error>> {
    let content = fs::read_to_string(path)?;
    let mut points = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        if idx == 0 && line.contains("x,y,timestamp_ms") {
            continue;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let mut parts = line.split(',');
        let x: i32 = parts.next().ok_or("missing x")?.trim().parse()?;
        let y: i32 = parts.next().ok_or("missing y")?.trim().parse()?;
        let timestamp_ms: u128 = parts.next().ok_or("missing timestamp_ms")?.trim().parse()?;
        points.push(Point {
            x,
            y,
            timestamp_ms,
        });
    }

    points.sort_by_key(|p| p.timestamp_ms);
    Ok(points)
}

fn build_segments(points: &[Point]) -> Result<Vec<MotionSegment>, Box<dyn Error>> {
    let mut segments = Vec::with_capacity(points.len());

    let first = &points[0];
    segments.push(MotionSegment {
        kind: PACKET_KIND_MOVE,
        dx_px: first.x,
        dy_px: first.y,
        dt_ms: 0,
        dx_mm: first.x as f32 * MM_PER_PX_X,
        dy_mm: first.y as f32 * MM_PER_PX_Y,
        speed_mm_s: 0.0,
        steps_x: pixels_to_steps_x(first.x as f32),
        steps_y: pixels_to_steps_y(first.y as f32),
    });

    for pair in points.windows(2) {
        let prev = &pair[0];
        let cur = &pair[1];
        let dx_px = cur.x - prev.x;
        let dy_px = cur.y - prev.y;
        let dt_ms_u128 = cur.timestamp_ms.saturating_sub(prev.timestamp_ms);
        let dt_ms = dt_ms_u128.min(u16::MAX as u128) as u16;

        let dx_mm = dx_px as f32 * MM_PER_PX_X;
        let dy_mm = dy_px as f32 * MM_PER_PX_Y;
        let dt_s = (dt_ms as f32 / 1000.0).max(f32::EPSILON);
        let distance_mm = (dx_mm * dx_mm + dy_mm * dy_mm).sqrt();
        let speed_mm_s = distance_mm / dt_s;

        segments.push(MotionSegment {
            kind: PACKET_KIND_DRAW,
            dx_px,
            dy_px,
            dt_ms,
            dx_mm,
            dy_mm,
            speed_mm_s,
            steps_x: pixels_to_steps_x(dx_px as f32),
            steps_y: pixels_to_steps_y(dy_px as f32),
        });
    }

    Ok(segments)
}

fn pixels_to_steps_x(px: f32) -> i16 {
    mm_to_steps(px * MM_PER_PX_X, STEPS_PER_MM_X)
}

fn pixels_to_steps_y(px: f32) -> i16 {
    mm_to_steps(px * MM_PER_PX_Y, STEPS_PER_MM_Y)
}

fn mm_to_steps(mm: f32, steps_per_mm: f32) -> i16 {
    let steps = (mm * steps_per_mm).round();
    steps.clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

fn encode_packet(segment: &MotionSegment) -> [u8; 10] {
    let packet = Packet {
        kind: segment.kind,
        steps_x: segment.steps_x,
        steps_y: segment.steps_y,
        dt_ms: segment.dt_ms,
    };
    let mut out = [0u8; 10];
    out[0] = PACKET_HEADER_1;
    out[1] = PACKET_HEADER_2;
    out[2] = packet.kind;
    out[3..5].copy_from_slice(&packet.steps_x.to_le_bytes());
    out[5..7].copy_from_slice(&packet.steps_y.to_le_bytes());
    out[7..9].copy_from_slice(&packet.dt_ms.to_le_bytes());
    out[9] = crc8(&out[2..9]);
    out
}

fn crc8(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |acc, &b| acc ^ b)
}

fn open_port(port: &str, baud: u32) -> Result<Box<dyn SerialPort>, Box<dyn Error>> {
    let port = serialport::new(port, baud)
        .timeout(Duration::from_millis(1000))
        .open()?;
    Ok(port)
}
