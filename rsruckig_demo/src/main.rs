use macroquad::prelude::*;
use rsruckig::prelude::*;
use std::collections::VecDeque;
use std::f32::consts::TAU;

const DOF: usize = 2;
const CONTROL_DT: f64 = 0.01;
const HOME_POSITION: [f64; DOF] = [0.0, 0.0];
const TARGET_POSITION: [f64; DOF] = [90.0, 90.0];
const WIDTH: i32 = 1100;
const HEIGHT: i32 = 760;
const TRAIL_LIMIT: usize = 4000;

const BG_COLOR: Color = Color {
    r: 0.957,
    g: 0.945,
    b: 0.918,
    a: 1.0,
};
const GRID_COLOR: Color = Color {
    r: 0.867,
    g: 0.839,
    b: 0.796,
    a: 1.0,
};
const AXIS_X_COLOR: Color = Color {
    r: 0.839,
    g: 0.271,
    b: 0.271,
    a: 1.0,
};
const AXIS_Y_COLOR: Color = Color {
    r: 0.247,
    g: 0.490,
    b: 0.290,
    a: 1.0,
};
const AXIS_Z_COLOR: Color = Color {
    r: 0.188,
    g: 0.314,
    b: 0.659,
    a: 1.0,
};
const SPHERE_COLOR: Color = Color {
    r: 0.541,
    g: 0.510,
    b: 0.459,
    a: 0.75,
};
const TRAJ_COLOR: Color = Color {
    r: 0.102,
    g: 0.102,
    b: 0.102,
    a: 1.0,
};
const CURRENT_COLOR: Color = Color {
    r: 0.839,
    g: 0.271,
    b: 0.271,
    a: 1.0,
};
const TARGET_COLOR: Color = Color {
    r: 0.188,
    g: 0.314,
    b: 0.659,
    a: 1.0,
};
const PANEL_BG: Color = Color {
    r: 0.902,
    g: 0.871,
    b: 0.824,
    a: 1.0,
};
const PANEL_BORDER: Color = Color {
    r: 0.580,
    g: 0.545,
    b: 0.498,
    a: 1.0,
};
const PANEL_GRID: Color = Color {
    r: 0.753,
    g: 0.718,
    b: 0.667,
    a: 1.0,
};
const TEXT_COLOR: Color = Color {
    r: 0.145,
    g: 0.137,
    b: 0.122,
    a: 1.0,
};

const PAN_TILT_PANEL_X: f32 = 30.0;
const PAN_TILT_PANEL_Y: f32 = 30.0;
const PAN_TILT_PANEL_W: f32 = 260.0;
const PAN_TILT_PANEL_H: f32 = 260.0;
const CAMERA_FOVY_DEG: f32 = 56.0;

fn window_conf() -> Conf {
    Conf {
        window_title: "rsruckig demo plotter - 3D camera rotation".to_string(),
        window_width: WIDTH,
        window_height: HEIGHT,
        window_resizable: true,
        sample_count: 4,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut motion = MotionState::new();

    loop {
        if is_key_pressed(KeyCode::Space) {
            motion.paused = !motion.paused;
        }
        if is_key_down(KeyCode::Escape) {
            break;
        }

        if !motion.paused {
            motion.update(get_frame_time() as f64);
        } else {
            motion.time_accumulator = 0.0;
        }

        draw_scene(&motion);
        next_frame().await;
    }
}

struct MotionState {
    otg: Ruckig<DOF, ThrowErrorHandler>,
    input: InputParameter<DOF>,
    output: OutputParameter<DOF>,
    current_position: [f64; DOF],
    current_velocity: [f64; DOF],
    current_acceleration: [f64; DOF],
    target_position: [f64; DOF],
    trail: VecDeque<Vec3>,
    pan_tilt_trail: VecDeque<[f64; DOF]>,
    paused: bool,
    time_accumulator: f64,
}

impl MotionState {
    fn new() -> Self {
        let mut input = InputParameter::<DOF>::new(None);
        let output = OutputParameter::<DOF>::new(None);
        let target_position = TARGET_POSITION;

        input.current_position[0] = HOME_POSITION[0];
        input.current_position[1] = HOME_POSITION[1];
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
        input.target_position[0] = target_position[0];
        input.target_position[1] = target_position[1];
        input.target_velocity[0] = 0.0;
        input.target_velocity[1] = 0.0;
        input.target_acceleration[0] = 0.0;
        input.target_acceleration[1] = 0.0;

        Self {
            otg: Ruckig::<DOF, ThrowErrorHandler>::new(None, CONTROL_DT),
            input,
            output,
            current_position: HOME_POSITION,
            current_velocity: [0.0, 0.0],
            current_acceleration: [0.0, 0.0],
            target_position,
            trail: VecDeque::with_capacity(TRAIL_LIMIT),
            pan_tilt_trail: VecDeque::with_capacity(TRAIL_LIMIT),
            paused: false,
            time_accumulator: 0.0,
        }
    }

    fn update(&mut self, frame_delta: f64) {
        if self.is_arrived() {
            self.time_accumulator = 0.0;
            return;
        }

        self.time_accumulator += frame_delta;

        while self.time_accumulator >= CONTROL_DT {
            self.input.current_position[0] = self.current_position[0];
            self.input.current_position[1] = self.current_position[1];
            self.input.current_velocity[0] = self.current_velocity[0];
            self.input.current_velocity[1] = self.current_velocity[1];
            self.input.current_acceleration[0] = self.current_acceleration[0];
            self.input.current_acceleration[1] = self.current_acceleration[1];
            self.input.target_position[0] = self.target_position[0];
            self.input.target_position[1] = self.target_position[1];

            self.otg
                .update(&self.input, &mut self.output)
                .expect("ruckig update failed");
            self.output.pass_to_input(&mut self.input);

            self.current_position = [self.output.new_position[0], self.output.new_position[1]];
            self.current_velocity = [self.output.new_velocity[0], self.output.new_velocity[1]];
            self.current_acceleration = [
                self.output.new_acceleration[0],
                self.output.new_acceleration[1],
            ];

            self.trail.push_back(ptz_direction(
                self.current_position[0],
                self.current_position[1],
            ));
            if self.trail.len() > TRAIL_LIMIT {
                self.trail.pop_front();
            }

            self.pan_tilt_trail.push_back(self.current_position);
            if self.pan_tilt_trail.len() > TRAIL_LIMIT {
                self.pan_tilt_trail.pop_front();
            }

            self.time_accumulator -= CONTROL_DT;
            if self.is_arrived() {
                self.time_accumulator = 0.0;
                break;
            }
        }
    }

    fn is_arrived(&self) -> bool {
        target_reached(
            &self.current_position,
            &self.current_velocity,
            &self.target_position,
        )
    }

    fn current_direction(&self) -> Vec3 {
        ptz_direction(self.current_position[0], self.current_position[1])
    }

    fn target_direction(&self) -> Vec3 {
        ptz_direction(self.target_position[0], self.target_position[1])
    }
}

fn target_reached(
    current_position: &[f64; DOF],
    current_velocity: &[f64; DOF],
    target_position: &[f64; DOF],
) -> bool {
    let pos_ok = (current_position[0] - target_position[0]).abs() < 0.05
        && (current_position[1] - target_position[1]).abs() < 0.05;
    let vel_ok = current_velocity[0].abs() < 0.05 && current_velocity[1].abs() < 0.05;
    pos_ok && vel_ok
}

fn draw_scene(motion: &MotionState) {
    clear_background(BG_COLOR);

    let scene_focus = motion.current_direction();
    set_camera(&Camera3D {
        position: scene_focus + vec3(3.2, 2.3, 4.8),
        up: vec3(0.0, 1.0, 0.0),
        target: scene_focus,
        fovy: CAMERA_FOVY_DEG.to_radians(),
        ..Default::default()
    });

    draw_floor_grid();
    draw_axes();
    draw_unit_sphere();
    draw_direction_trail(&motion.trail);
    draw_camera_rotation(motion.current_direction(), motion.target_direction());

    set_default_camera();
    draw_pan_tilt_panel(
        &motion.pan_tilt_trail,
        motion.current_position,
        motion.target_position,
    );
    draw_status(motion);
}

fn draw_floor_grid() {
    for step in (-6..=6).map(|v| v as f32 * 0.25) {
        draw_line_3d(vec3(step, -1.25, -1.5), vec3(step, -1.25, 1.5), GRID_COLOR);
        draw_line_3d(vec3(-1.5, -1.25, step), vec3(1.5, -1.25, step), GRID_COLOR);
    }
}

fn draw_axes() {
    let origin = vec3(0.0, 0.0, 0.0);
    draw_line_3d(origin, vec3(1.25, 0.0, 0.0), AXIS_X_COLOR);
    draw_line_3d(origin, vec3(0.0, 1.25, 0.0), AXIS_Y_COLOR);
    draw_line_3d(origin, vec3(0.0, 0.0, 1.25), AXIS_Z_COLOR);
}

fn draw_unit_sphere() {
    draw_great_circle(|a| vec3(a.cos(), 0.0, a.sin()), SPHERE_COLOR);
    draw_great_circle(|a| vec3(0.0, a.cos(), a.sin()), SPHERE_COLOR);
    draw_great_circle(|a| vec3(a.cos(), a.sin(), 0.0), SPHERE_COLOR);
}

fn draw_great_circle<F>(mapper: F, color: Color)
where
    F: Fn(f32) -> Vec3,
{
    let steps = 128;
    let mut prev = mapper(0.0);
    for i in 1..=steps {
        let t = TAU * (i as f32 / steps as f32);
        let next = mapper(t);
        draw_line_3d(prev, next, color);
        prev = next;
    }
}

fn draw_direction_trail(trail: &VecDeque<Vec3>) {
    let origin = vec3(0.0, 0.0, 0.0);
    let mut prev = origin;

    for point in trail.iter().copied() {
        draw_line_3d(prev, point, TRAJ_COLOR);
        prev = point;
    }
}

fn draw_camera_rotation(current: Vec3, target: Vec3) {
    let origin = vec3(0.0, 0.0, 0.0);

    draw_line_3d(origin, target, TARGET_COLOR);
    draw_sphere(target, 0.035, None, TARGET_COLOR);

    draw_line_3d(origin, current, CURRENT_COLOR);
    draw_camera_frustum(current, CURRENT_COLOR);
    draw_sphere(current, 0.05, None, CURRENT_COLOR);
}

fn draw_camera_frustum(direction: Vec3, color: Color) {
    let origin = vec3(0.0, 0.0, 0.0);
    let forward = direction.normalize();
    let up_hint = if forward.y.abs() > 0.95 {
        vec3(1.0, 0.0, 0.0)
    } else {
        vec3(0.0, 1.0, 0.0)
    };
    let right = forward.cross(up_hint).normalize();
    let up = right.cross(forward).normalize();
    let plane_center = origin + forward * 0.34;
    let half_width = 0.13;
    let half_height = 0.09;
    let corners = [
        plane_center + right * half_width + up * half_height,
        plane_center - right * half_width + up * half_height,
        plane_center - right * half_width - up * half_height,
        plane_center + right * half_width - up * half_height,
    ];

    for corner in corners {
        draw_line_3d(origin, corner, color);
    }
    for i in 0..corners.len() {
        draw_line_3d(corners[i], corners[(i + 1) % corners.len()], color);
    }
}

fn draw_pan_tilt_panel(trail: &VecDeque<[f64; DOF]>, current: [f64; DOF], target: [f64; DOF]) {
    draw_rectangle(
        PAN_TILT_PANEL_X,
        PAN_TILT_PANEL_Y,
        PAN_TILT_PANEL_W,
        PAN_TILT_PANEL_H,
        PANEL_BG,
    );
    draw_rectangle_lines(
        PAN_TILT_PANEL_X,
        PAN_TILT_PANEL_Y,
        PAN_TILT_PANEL_W,
        PAN_TILT_PANEL_H,
        1.5,
        PANEL_BORDER,
    );

    let center_x = PAN_TILT_PANEL_X + PAN_TILT_PANEL_W * 0.5;
    let center_y = PAN_TILT_PANEL_Y + PAN_TILT_PANEL_H * 0.5;
    draw_line(
        center_x,
        PAN_TILT_PANEL_Y,
        center_x,
        PAN_TILT_PANEL_Y + PAN_TILT_PANEL_H,
        1.0,
        PANEL_GRID,
    );
    draw_line(
        PAN_TILT_PANEL_X,
        center_y,
        PAN_TILT_PANEL_X + PAN_TILT_PANEL_W,
        center_y,
        1.0,
        PANEL_GRID,
    );

    let mut prev: Option<Vec2> = None;
    for point in trail.iter().copied() {
        let p = pan_tilt_to_panel(point);
        if let Some(last) = prev {
            draw_line(last.x, last.y, p.x, p.y, 2.0, TRAJ_COLOR);
        }
        prev = Some(p);
    }

    let current_pt = pan_tilt_to_panel(current);
    let target_pt = pan_tilt_to_panel(target);
    draw_circle(target_pt.x, target_pt.y, 4.0, TARGET_COLOR);
    draw_circle(current_pt.x, current_pt.y, 5.5, CURRENT_COLOR);
}

fn draw_status(motion: &MotionState) {
    let status = if motion.is_arrived() {
        "arrived"
    } else if motion.paused {
        "paused"
    } else {
        "running"
    };
    let values = format!(
        "{status}   pan {:>6.1}   tilt {:>6.1}   target {:>6.1}/{:>6.1}",
        motion.current_position[0],
        motion.current_position[1],
        motion.target_position[0],
        motion.target_position[1]
    );
    draw_text(&values, 30.0, screen_height() - 28.0, 22.0, TEXT_COLOR);
}

fn pan_tilt_to_panel(point: [f64; DOF]) -> Vec2 {
    let pan = (point[0] / 360.0).clamp(0.0, 1.0) as f32;
    let tilt = (point[1] / 360.0).clamp(0.0, 1.0) as f32;
    vec2(
        PAN_TILT_PANEL_X + pan * PAN_TILT_PANEL_W,
        PAN_TILT_PANEL_Y + (1.0 - tilt) * PAN_TILT_PANEL_H,
    )
}

fn ptz_direction(pan_deg: f64, tilt_deg: f64) -> Vec3 {
    let pan = pan_deg.to_radians() as f32;
    let tilt = tilt_deg.to_radians() as f32;

    vec3(tilt.cos() * pan.sin(), tilt.sin(), tilt.cos() * pan.cos())
}
