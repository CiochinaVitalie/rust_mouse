use minifb::{Key, Window, WindowOptions};
use rsruckig::prelude::*;
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::time::{Duration, Instant};

const DOF: usize = 2;
const CONTROL_DT: f64 = 0.01;
const WIDTH: usize = 1100;
const HEIGHT: usize = 760;
const BG_COLOR: u32 = 0x00F4F1EA;
const GRID_COLOR: u32 = 0x00DDD6CB;
const AXIS_X_COLOR: u32 = 0x00D64545;
const AXIS_Y_COLOR: u32 = 0x003F7D4A;
const AXIS_Z_COLOR: u32 = 0x003050A8;
const SPHERE_COLOR: u32 = 0x00C8C0B4;
const TRAJ_COLOR: u32 = 0x001A1A1A;
const CURRENT_COLOR: u32 = 0x00D64545;
const TARGET_COLOR: u32 = 0x003050A8;
const WORLD_SCALE: f64 = 250.0;
const CAMERA_DISTANCE: f64 = 4.0;
const FOCAL_LENGTH: f64 = 1.8;
const VIEW_YAW: f64 = 0.85;
const VIEW_PITCH: f64 = -0.35;
const TRAIL_LIMIT: usize = 4000;
const PAN_TILT_PANEL_X: i32 = 30;
const PAN_TILT_PANEL_Y: i32 = 30;
const PAN_TILT_PANEL_W: i32 = 260;
const PAN_TILT_PANEL_H: i32 = 260;

#[derive(Debug, Clone, Copy)]
struct Vec3 {
    x: f64,
    y: f64,
    z: f64,
}

impl Vec3 {
    fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

use std::ops::{Add, Mul, Sub};

impl Add for Vec3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut window = Window::new(
        "rsruckig demo plotter - autonomous 360/360 round trip, Esc exit",
        WIDTH,
        HEIGHT,
        WindowOptions::default(),
    )?;

    let mut buffer = vec![BG_COLOR; WIDTH * HEIGHT];
    let mut otg = Ruckig::<DOF, ThrowErrorHandler>::new(None, CONTROL_DT);
    let mut input = InputParameter::<DOF>::new(None);
    let mut output = OutputParameter::<DOF>::new(None);

    input.current_position[0] = 0.0;
    input.current_position[1] = 0.0;
    input.current_velocity[0] = 0.0;
    input.current_velocity[1] = 0.0;
    input.current_acceleration[0] = 0.0;
    input.current_acceleration[1] = 0.0;
    input.max_velocity[0] = 50.0;
    input.max_velocity[1] = 50.0;
    input.max_acceleration[0] = 120.0;
    input.max_acceleration[1] = 120.0;
    input.max_jerk[0] = 600.0;
    input.max_jerk[1] = 600.0;
    input.target_position[0] = 0.0;
    input.target_position[1] = 0.0;
    input.target_velocity[0] = 0.0;
    input.target_velocity[1] = 0.0;
    input.target_acceleration[0] = 0.0;
    input.target_acceleration[1] = 0.0;

    let mut current_position = [0.0, 0.0];
    let mut current_velocity = [0.0, 0.0];
    let mut current_acceleration = [0.0, 0.0];
    let mut target_position = [0.0, 360.0];
    let mut returning = false;
    input.target_position[0] = target_position[0];
    input.target_position[1] = target_position[1];
    let mut trail: VecDeque<Vec3> = VecDeque::with_capacity(TRAIL_LIMIT);
    let mut pan_tilt_trail: VecDeque<[f64; DOF]> = VecDeque::with_capacity(TRAIL_LIMIT);
    let mut paused = false;
    let mut last_tick = Instant::now();
    let mut time_accumulator = Duration::ZERO;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        if window.is_key_pressed(Key::Space, minifb::KeyRepeat::No) {
            paused = !paused;
            last_tick = Instant::now();
        }

        let now = Instant::now();
        let frame_delta = now.duration_since(last_tick);
        last_tick = now;

        if !paused {
            time_accumulator += frame_delta;

            if target_reached(&current_position, &current_velocity, &target_position) {
                returning = !returning;
                target_position = if returning {
                    [0.0, 0.0]
                } else {
                    [360.0, 360.0]
                };
                input.target_position[0] = target_position[0];
                input.target_position[1] = target_position[1];
            }

            let control_step = Duration::from_secs_f64(CONTROL_DT);
            while time_accumulator >= control_step {
                input.current_position[0] = current_position[0];
                input.current_position[1] = current_position[1];
                input.current_velocity[0] = current_velocity[0];
                input.current_velocity[1] = current_velocity[1];
                input.current_acceleration[0] = current_acceleration[0];
                input.current_acceleration[1] = current_acceleration[1];
                input.target_position[0] = target_position[0];
                input.target_position[1] = target_position[1];

                otg.update(&input, &mut output)?;
                output.pass_to_input(&mut input);

                current_position = [output.new_position[0], output.new_position[1]];
                current_velocity = [output.new_velocity[0], output.new_velocity[1]];
                current_acceleration = [output.new_acceleration[0], output.new_acceleration[1]];

                let direction = ptz_direction(current_position[0], current_position[1]);
                trail.push_back(direction);
                if trail.len() > TRAIL_LIMIT {
                    trail.pop_front();
                }

                pan_tilt_trail.push_back(current_position);
                if pan_tilt_trail.len() > TRAIL_LIMIT {
                    pan_tilt_trail.pop_front();
                }

                time_accumulator -= control_step;
            }
        } else {
            time_accumulator = Duration::ZERO;
        }

        buffer.fill(BG_COLOR);
        draw_scene(
            &mut buffer,
            &trail,
            &pan_tilt_trail,
            ptz_direction(current_position[0], current_position[1]),
            ptz_direction(target_position[0], target_position[1]),
            current_position,
            target_position,
        );

        window.update_with_buffer(&buffer, WIDTH, HEIGHT)?;
    }

    Ok(())
}

fn target_reached(current_position: &[f64; DOF], current_velocity: &[f64; DOF], target_position: &[f64; DOF]) -> bool {
    let pos_ok = (current_position[0] - target_position[0]).abs() < 0.05
        && (current_position[1] - target_position[1]).abs() < 0.05;
    let vel_ok = current_velocity[0].abs() < 0.05 && current_velocity[1].abs() < 0.05;
    pos_ok && vel_ok
}

fn draw_scene(
    buffer: &mut [u32],
    trail: &VecDeque<Vec3>,
    pan_tilt_trail: &VecDeque<[f64; DOF]>,
    current: Vec3,
    target: Vec3,
    current_pan_tilt: [f64; DOF],
    target_pan_tilt: [f64; DOF],
) {
    draw_grid(buffer);
    draw_axes(buffer);
    draw_sphere(buffer);
    draw_pan_tilt_panel(buffer, pan_tilt_trail, current_pan_tilt, target_pan_tilt);

    let origin = Vec3::zero();
    let mut prev = origin;
    for point in trail.iter().copied() {
        draw_line_3d(buffer, prev, point, TRAJ_COLOR);
        prev = point;
    }

    draw_line_3d(buffer, origin, current, CURRENT_COLOR);
    draw_circle_3d(buffer, current, 6, CURRENT_COLOR);
    draw_line_3d(buffer, origin, target, TARGET_COLOR);
    draw_circle_3d(buffer, target, 4, TARGET_COLOR);
}

fn draw_pan_tilt_panel(
    buffer: &mut [u32],
    trail: &VecDeque<[f64; DOF]>,
    current: [f64; DOF],
    target: [f64; DOF],
) {
    let x0 = PAN_TILT_PANEL_X;
    let y0 = PAN_TILT_PANEL_Y;
    let x1 = x0 + PAN_TILT_PANEL_W;
    let y1 = y0 + PAN_TILT_PANEL_H;

    draw_rect(buffer, x0, y0, x1, y1, 0x00E6DED2);
    draw_line(buffer, (x0, y0), (x1, y0), 0x00948B7F);
    draw_line(buffer, (x1, y0), (x1, y1), 0x00948B7F);
    draw_line(buffer, (x1, y1), (x0, y1), 0x00948B7F);
    draw_line(buffer, (x0, y1), (x0, y0), 0x00948B7F);

    let center_x = x0 + PAN_TILT_PANEL_W / 2;
    let center_y = y0 + PAN_TILT_PANEL_H / 2;
    draw_line(buffer, (center_x, y0), (center_x, y1), 0x00C0B7AA);
    draw_line(buffer, (x0, center_y), (x1, center_y), 0x00C0B7AA);

    let mut prev = None;
    for point in trail.iter().copied() {
        let p = pan_tilt_to_panel(point);
        if let Some(last) = prev {
            draw_line(buffer, last, p, TRAJ_COLOR);
        }
        prev = Some(p);
    }

    let current_pt = pan_tilt_to_panel(current);
    let target_pt = pan_tilt_to_panel(target);
    draw_circle(buffer, current_pt, 4, CURRENT_COLOR);
    draw_circle(buffer, target_pt, 3, TARGET_COLOR);
}

fn pan_tilt_to_panel(point: [f64; DOF]) -> (i32, i32) {
    let pan = (point[0] / 360.0).clamp(0.0, 1.0);
    let tilt = (point[1] / 360.0).clamp(0.0, 1.0);
    let x = PAN_TILT_PANEL_X as f64 + pan * PAN_TILT_PANEL_W as f64;
    let y = PAN_TILT_PANEL_Y as f64 + (1.0 - tilt) * PAN_TILT_PANEL_H as f64;
    (x.round() as i32, y.round() as i32)
}

fn draw_rect(buffer: &mut [u32], x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
    for y in y0.max(0)..=y1.min((HEIGHT as i32) - 1) {
        for x in x0.max(0)..=x1.min((WIDTH as i32) - 1) {
            plot(buffer, x, y, color);
        }
    }
}

fn draw_circle(buffer: &mut [u32], center: (i32, i32), radius_px: i32, color: u32) {
    for dy in -radius_px..=radius_px {
        for dx in -radius_px..=radius_px {
            if dx * dx + dy * dy <= radius_px * radius_px {
                plot(buffer, center.0 + dx, center.1 + dy, color);
            }
        }
    }
}

fn ptz_direction(pan_deg: f64, tilt_deg: f64) -> Vec3 {
    let pan = pan_deg.to_radians();
    let tilt = tilt_deg.to_radians();

    Vec3::new(
        tilt.cos() * pan.sin(),
        tilt.sin(),
        tilt.cos() * pan.cos(),
    )
}

fn draw_axes(buffer: &mut [u32]) {
    let origin = Vec3::zero();
    draw_axis_line(buffer, origin, Vec3::new(1.2, 0.0, 0.0), AXIS_X_COLOR);
    draw_axis_line(buffer, origin, Vec3::new(0.0, 1.2, 0.0), AXIS_Y_COLOR);
    draw_axis_line(buffer, origin, Vec3::new(0.0, 0.0, 1.2), AXIS_Z_COLOR);
    draw_circle_3d(buffer, origin, 4, AXIS_Z_COLOR);
}

fn draw_axis_line(buffer: &mut [u32], start: Vec3, end: Vec3, color: u32) {
    draw_line_3d(buffer, start, end, color);
}

fn draw_sphere(buffer: &mut [u32]) {
    draw_great_circle(buffer, |a| Vec3::new(a.cos(), 0.0, a.sin()), SPHERE_COLOR);
    draw_great_circle(buffer, |a| Vec3::new(0.0, a.cos(), a.sin()), SPHERE_COLOR);
    draw_great_circle(buffer, |a| Vec3::new(a.cos(), a.sin(), 0.0), SPHERE_COLOR);
}

fn draw_great_circle<F>(buffer: &mut [u32], mapper: F, color: u32)
where
    F: Fn(f64) -> Vec3,
{
    let steps = 96;
    let mut prev = mapper(0.0);
    for i in 1..=steps {
        let t = TAU * (i as f64 / steps as f64);
        let next = mapper(t);
        draw_line_3d(buffer, prev, next, color);
        prev = next;
    }
}

fn draw_line_3d(buffer: &mut [u32], start: Vec3, end: Vec3, color: u32) {
    if let (Some(a), Some(b)) = (project(start), project(end)) {
        draw_line(buffer, a, b, color);
    }
}

fn draw_circle_3d(buffer: &mut [u32], center: Vec3, radius_px: i32, color: u32) {
    if let Some((cx, cy)) = project(center) {
        for dy in -radius_px..=radius_px {
            for dx in -radius_px..=radius_px {
                if dx * dx + dy * dy <= radius_px * radius_px {
                    plot(buffer, cx + dx, cy + dy, color);
                }
            }
        }
    }
}

fn project(point: Vec3) -> Option<(i32, i32)> {
    let rotated = rotate_x(rotate_y(point, VIEW_YAW), VIEW_PITCH);
    let depth = CAMERA_DISTANCE - rotated.z;
    if depth <= 0.15 {
        return None;
    }

    let perspective = FOCAL_LENGTH / depth;
    let screen_x = (WIDTH as f64 * 0.5) + rotated.x * perspective * WORLD_SCALE;
    let screen_y = (HEIGHT as f64 * 0.54) - rotated.y * perspective * WORLD_SCALE;
    Some((screen_x.round() as i32, screen_y.round() as i32))
}

fn rotate_y(point: Vec3, angle: f64) -> Vec3 {
    let s = angle.sin();
    let c = angle.cos();
    Vec3::new(point.x * c + point.z * s, point.y, -point.x * s + point.z * c)
}

fn rotate_x(point: Vec3, angle: f64) -> Vec3 {
    let s = angle.sin();
    let c = angle.cos();
    Vec3::new(point.x, point.y * c - point.z * s, point.y * s + point.z * c)
}

fn draw_grid(buffer: &mut [u32]) {
    for step in (-6..=6).map(|v| v as f64 * 0.25) {
        let x0 = Vec3::new(step, -1.3, 1.5);
        let x1 = Vec3::new(step, 1.3, 1.5);
        draw_line_3d(buffer, x0, x1, GRID_COLOR);

        let z0 = Vec3::new(-1.3, -1.3, step);
        let z1 = Vec3::new(1.3, -1.3, step);
        draw_line_3d(buffer, z0, z1, GRID_COLOR);
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
