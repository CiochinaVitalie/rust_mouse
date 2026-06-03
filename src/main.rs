use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use std::collections::VecDeque;
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const WIDTH: usize = 900;
const HEIGHT: usize = 600;
const BG_COLOR: u32 = 0x00F4F1EA;
const DRAW_COLOR: u32 = 0x001A1A1A;
const SAMPLE_BUFFER_LIMIT: usize = 2_000;
const SAMPLE_PERIOD: Duration = Duration::from_millis(10);

#[derive(Debug, Clone)]
struct TrajectoryPoint {
    x: i32,
    y: i32,
    timestamp_ms: u128,
}

fn main() -> Result<(), minifb::Error> {
    let mut window = Window::new(
        "Rust Mouse Drawer - hold left mouse button to draw, press C to clear",
        WIDTH,
        HEIGHT,
        WindowOptions::default(),
    )?;

    let mut buffer = vec![BG_COLOR; WIDTH * HEIGHT];
    let mut samples: VecDeque<TrajectoryPoint> = VecDeque::with_capacity(SAMPLE_BUFFER_LIMIT);
    let mut csv_writer = create_csv_writer().ok();
    let mut last_point: Option<(i32, i32)> = None;
    let mut next_sample_time = Instant::now();
    let start_time = Instant::now();
    let mut was_down = false;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        if window.is_key_pressed(Key::C, minifb::KeyRepeat::No) {
            buffer.fill(BG_COLOR);
            samples.clear();
            last_point = None;
            next_sample_time = Instant::now();
        }

        if window.is_key_pressed(Key::B, minifb::KeyRepeat::No) {
            dump_recent_samples(&samples, 10);
        }

        if window.is_key_pressed(Key::S, minifb::KeyRepeat::No) {
            if let Some(writer) = csv_writer.as_mut() {
                if let Err(err) = writer.flush() {
                    eprintln!("failed to flush csv: {}", err);
                } else {
                    println!("csv flushed");
                }
            }
        }

        let is_down = window.get_mouse_down(MouseButton::Left);
        if let Some((mx, my)) = window.get_mouse_pos(MouseMode::Discard) {
            let point = (mx as i32, my as i32);
            let now = Instant::now();

            if is_down {
                if was_down {
                    if let Some(prev) = last_point {
                        draw_line(&mut buffer, prev, point, DRAW_COLOR);
                    } else {
                        draw_brush(&mut buffer, point, DRAW_COLOR);
                    }
                } else {
                    draw_brush(&mut buffer, point, DRAW_COLOR);
                }

                if now >= next_sample_time {
                    samples.push_back(TrajectoryPoint {
                        x: point.0,
                        y: point.1,
                        timestamp_ms: now.duration_since(start_time).as_millis(),
                    });
                    if samples.len() > SAMPLE_BUFFER_LIMIT {
                        samples.pop_front();
                    }

                    println!(
                        "x: {}, y: {}, t: {} ms",
                        point.0,
                        point.1,
                        now.duration_since(start_time).as_millis()
                    );

                    if let Some(writer) = csv_writer.as_mut() {
                        if let Err(err) = writeln!(
                            writer,
                            "{},{},{}",
                            point.0,
                            point.1,
                            now.duration_since(start_time).as_millis()
                        ) {
                            eprintln!("failed to write csv row: {}", err);
                            csv_writer = None;
                        } else if let Err(err) = writer.flush() {
                            eprintln!("failed to flush csv row: {}", err);
                            csv_writer = None;
                        }
                    }

                    next_sample_time = now + SAMPLE_PERIOD;
                }
                last_point = Some(point);
            } else {
                last_point = None;
            }
        } else if !is_down {
            last_point = None;
            next_sample_time = Instant::now();
        }
        was_down = is_down;

        window.update_with_buffer(&buffer, WIDTH, HEIGHT)?;
    }

    Ok(())
}

fn dump_recent_samples(samples: &VecDeque<TrajectoryPoint>, count: usize) {
    let start = samples.len().saturating_sub(count);
    for sample in samples.iter().skip(start) {
        println!(
            "x: {}, y: {}, t: {} ms",
            sample.x, sample.y, sample.timestamp_ms
        );
    }
}

fn create_csv_writer() -> std::io::Result<BufWriter<File>> {
    let output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("output");
    create_dir_all(&output_dir)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let path = output_dir.join(format!("trajectory_{}.csv", timestamp));
    let file = File::create(&path)?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "x,y,timestamp_ms")?;
    writer.flush()?;
    println!("csv file created: {}", path.display());
    Ok(writer)
}

fn draw_brush(buffer: &mut [u32], point: (i32, i32), color: u32) {
    let radius = 0;
    for y in -radius..=radius {
        for x in -radius..=radius {
            plot(buffer, point.0 + x, point.1 + y, color);
        }
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
        draw_brush(buffer, (x0, y0), color);
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
