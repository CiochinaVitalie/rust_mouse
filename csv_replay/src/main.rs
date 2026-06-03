use minifb::{Key, Window, WindowOptions};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const WIDTH: usize = 900;
const HEIGHT: usize = 600;
const BG_COLOR: u32 = 0x00F4F1EA;
const DRAW_COLOR: u32 = 0x001A1A1A;

#[derive(Debug, Clone)]
struct Point {
    x: i32,
    y: i32,
    timestamp_ms: u128,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let csv_path = resolve_csv_path()?;
    let points = load_points(&csv_path)?;
    if points.is_empty() {
        return Err("CSV does not contain any points".into());
    }

    let mut window = Window::new(
        &format!("CSV Replay - {}", csv_path.display()),
        WIDTH,
        HEIGHT,
        WindowOptions::default(),
    )?;

    let mut buffer = vec![BG_COLOR; WIDTH * HEIGHT];
    let mut replay_start = Instant::now();
    let mut paused = false;
    let mut pause_started: Option<Instant> = None;
    let mut paused_total = Duration::ZERO;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        if window.is_key_pressed(Key::Space, minifb::KeyRepeat::No) {
            if paused {
                if let Some(start) = pause_started.take() {
                    paused_total += start.elapsed();
                }
            } else {
                pause_started = Some(Instant::now());
            }
            paused = !paused;
        }

        if window.is_key_pressed(Key::R, minifb::KeyRepeat::No) {
            buffer.fill(BG_COLOR);
            replay_start = Instant::now();
            paused_total = Duration::ZERO;
            paused = false;
            pause_started = None;
        }

        let elapsed = if paused {
            pause_started
                .map(|start| start.duration_since(replay_start) - paused_total)
                .unwrap_or(Duration::ZERO)
        } else {
            Instant::now()
                .duration_since(replay_start)
                .saturating_sub(paused_total)
        };
        let playback_ms = elapsed.as_millis();

        buffer.fill(BG_COLOR);
        draw_axes(&mut buffer);

        let mut prev: Option<(i32, i32)> = None;
        for point in points.iter().take_while(|p| p.timestamp_ms <= playback_ms) {
            let pos = (point.x, point.y);
            if let Some(last) = prev {
                draw_line(&mut buffer, last, pos, DRAW_COLOR);
            }
            prev = Some(pos);
        }

        window.update_with_buffer(&buffer, WIDTH, HEIGHT)?;
    }

    Ok(())
}

fn resolve_csv_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(arg) = env::args().nth(1) {
        return Ok(PathBuf::from(arg));
    }

    for dir in ["output", "../output"] {
        if let Some(path) = newest_csv_in(dir)? {
            return Ok(path);
        }
    }

    Err("no CSV path provided and no trajectory_*.csv found in output/ or ../output/".into())
}

fn newest_csv_in(dir: impl AsRef<Path>) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
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

        let meta = entry.metadata()?;
        let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        match &newest {
            Some((best_time, _)) if modified <= *best_time => {}
            _ => newest = Some((modified, path)),
        }
    }

    Ok(newest.map(|(_, path)| path))
}

fn load_points(path: &Path) -> Result<Vec<Point>, Box<dyn std::error::Error>> {
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

fn draw_axes(buffer: &mut [u32]) {
    let mid_x = WIDTH / 2;
    let mid_y = HEIGHT / 2;
    for x in 0..WIDTH {
        buffer[mid_y * WIDTH + x] = 0x00D8D2C8;
    }
    for y in 0..HEIGHT {
        buffer[y * WIDTH + mid_x] = 0x00D8D2C8;
    }
}

fn draw_line(buffer: &mut [u32], start: (i32, i32), end: (i32, i32), color: u32) {
    let mut x0 = start.0;
    let mut y0 = start.1;
    let x1 = end.0;
    let y1 = end.1;

    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        plot(buffer, x0, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn plot(buffer: &mut [u32], x: i32, y: i32, color: u32) {
    if x < 0 || y < 0 {
        return;
    }

    let x = x as usize;
    let y = y as usize;
    if x >= WIDTH || y >= HEIGHT {
        return;
    }

    buffer[y * WIDTH + x] = color;
}
