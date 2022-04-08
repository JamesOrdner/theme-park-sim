use std::f32::consts::{FRAC_PI_2, TAU};

use event::{AsyncEventDelegate, InputEvent};
use frame_buffer::{CameraInfo, FrameBufferWriter};
use nalgebra_glm::{vec3, Vec3};

pub struct FrameData {
    origin: Vec3,
    azimuth_angle: f32,
    polar_angle: f32,
    boom_len: f32,
}

impl Default for FrameData {
    fn default() -> Self {
        Self {
            origin: Default::default(),
            azimuth_angle: 0.0,
            polar_angle: 0.0,
            boom_len: 5.0,
        }
    }
}

const CAMERA_SPEED: f32 = 3.0;

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
                    let dir = nalgebra_glm::rotate_vec3(
                        &vec3(axis.x, 0.0, axis.y),
                        self.azimuth_angle,
                        &vec3(0.0, 1.0, 0.0),
                    );
                    self.origin += dir * CAMERA_SPEED * delta_time;
                }
                InputEvent::CameraRotateAxis(axis) => {
                    self.azimuth_angle += axis.x * 0.005;
                    self.azimuth_angle %= TAU;
                    self.polar_angle += axis.y * 0.01;
                    self.polar_angle = self.polar_angle.max(0.05).min(FRAC_PI_2 - 0.05);
                }
                _ => {}
            }
        }

        let location = nalgebra_glm::rotate_vec3(
            &vec3(0.0, 0.0, -self.boom_len),
            self.polar_angle,
            &vec3(1.0, 0.0, 0.0),
        );

        let location =
            nalgebra_glm::rotate_vec3(&location, self.azimuth_angle, &vec3(0.0, 1.0, 0.0));

        let location = self.origin + location;

        let camera_info = CameraInfo {
            focus: self.origin,
            location,
            up: vec3(0.0, 1.0, 0.0),
        };

        frame_buffer.set_camera_info(camera_info);
    }
}
