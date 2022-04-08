use std::f32::consts::FRAC_PI_2;

use event::{AsyncEventDelegate, FrameEvent, InputEvent};
use frame_buffer::{CameraInfo, FrameBufferWriter};
use nalgebra_glm::{rotate_vec3, vec3, Vec3};

pub struct FrameData {
    origin: Vec3,
    origin_vel: Vec3,
    azimuth_angle: f32,
    azimuth_angle_target: f32,
    polar_angle: f32,
    polar_angle_target: f32,
    boom_len: f32,
}

impl Default for FrameData {
    fn default() -> Self {
        Self {
            origin: Default::default(),
            origin_vel: Default::default(),
            azimuth_angle: 0.0,
            azimuth_angle_target: 0.0,
            polar_angle: 0.0,
            polar_angle_target: 0.0,
            boom_len: 5.0,
        }
    }
}

const MOVE_SPEED: f32 = 3.0;
const ROTATE_SPEED: f32 = 0.01;
const MOVE_DAMPING_FACTOR: f32 = 0.001;
const ROTATE_DAMPING_FACTOR: f32 = 0.00001;

impl FrameData {
    pub async fn update(
        &mut self,
        event_delegate: &AsyncEventDelegate<'_>,
        frame_buffer: &FrameBufferWriter<'_>,
        delta_time: f32,
    ) {
        for input_event in event_delegate.input_events() {
            match input_event {
                InputEvent::CameraMoveAxis(axis) => {
                    self.origin_vel += rotate_vec3(
                        &vec3(axis.x, 0.0, axis.y),
                        self.azimuth_angle_target,
                        &vec3(0.0, 1.0, 0.0),
                    );

                    let norm = self.origin_vel.norm();
                    if norm > 1.0 {
                        self.origin_vel.unscale_mut(norm);
                    }
                }
                InputEvent::CameraRotateAxis(axis) => {
                    self.azimuth_angle_target += axis.x * ROTATE_SPEED;
                    self.polar_angle_target += axis.y * ROTATE_SPEED;

                    self.polar_angle_target =
                        self.polar_angle_target.max(0.05).min(FRAC_PI_2 - 0.05);
                }
                InputEvent::CameraZoom(delta) => {
                    self.boom_len -= delta * 0.01;
                    self.boom_len = self.boom_len.max(1.0).min(15.0);
                }
                _ => {}
            }
        }

        self.origin += self.origin_vel * MOVE_SPEED * delta_time;
        self.origin_vel *= MOVE_DAMPING_FACTOR.powf(delta_time);

        let rotate_alpha = 1.0 - ROTATE_DAMPING_FACTOR.powf(delta_time);
        self.azimuth_angle += (self.azimuth_angle_target - self.azimuth_angle) * rotate_alpha;
        self.polar_angle += (self.polar_angle_target - self.polar_angle) * rotate_alpha;

        let location = rotate_vec3(
            &vec3(0.0, 0.0, -self.boom_len),
            self.polar_angle,
            &vec3(1.0, 0.0, 0.0),
        );
        let location = rotate_vec3(&location, self.azimuth_angle, &vec3(0.0, 1.0, 0.0));
        let location = self.origin + location;

        let orientation = (self.origin - location).normalize();

        event_delegate.push_frame_event(FrameEvent::CameraLocation(location));
        event_delegate.push_frame_event(FrameEvent::CameraOrientation(orientation));

        let camera_info = CameraInfo {
            focus: self.origin,
            location,
            up: vec3(0.0, 1.0, 0.0),
        };

        frame_buffer.set_camera_info(camera_info);
    }
}
