use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};

const WIDTH: usize = 900;
const HEIGHT: usize = 600;
const BG_COLOR: u32 = 0x00F4F1EA;
const DRAW_COLOR: u32 = 0x001A1A1A;

fn main() -> Result<(), minifb::Error> {
    let mut window = Window::new(
        "Rust Mouse Drawer - hold left mouse button to draw, press C to clear",
        WIDTH,
        HEIGHT,
        WindowOptions::default(),
    )?;

    let mut buffer = vec![BG_COLOR; WIDTH * HEIGHT];
    let mut last_point: Option<(i32, i32)> = None;
    let mut was_down = false;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        if window.is_key_pressed(Key::C, minifb::KeyRepeat::No) {
            buffer.fill(BG_COLOR);
            last_point = None;
        }

        let is_down = window.get_mouse_down(MouseButton::Left);
        if is_down {
            if let Some((mx, my)) = window.get_mouse_pos(MouseMode::Discard) {
                let point = (mx as i32, my as i32);

                if was_down {
                    if let Some(prev) = last_point {
                        draw_line(&mut buffer, prev, point, DRAW_COLOR);
                    } else {
                        draw_brush(&mut buffer, point, DRAW_COLOR);
                    }
                } else {
                    draw_brush(&mut buffer, point, DRAW_COLOR);
                }

                last_point = Some(point);
            }
        } else {
            last_point = None;
        }
        was_down = is_down;

        window.update_with_buffer(&buffer, WIDTH, HEIGHT)?;
    }

    Ok(())
}

fn draw_brush(buffer: &mut [u32], point: (i32, i32), color: u32) {
    let radius = 2;
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
