use std::f32::consts::FRAC_PI_2;

use event::{AsyncEventDelegate, FrameEvent, InputEvent};
use frame_buffer::{CameraInfo, FrameBufferWriter};
use nalgebra_glm::{rotate_vec3, vec3, Vec3};
use system_interfaces::static_mesh::Interface as StaticMeshInterface;

pub struct FrameData {
    static_mesh_interface: StaticMeshInterface,
    origin: Vec3,
    origin_vel: Vec3,
    azimuth_angle: f32,
    azimuth_angle_target: f32,
    polar_angle: f32,
    polar_angle_target: f32,
    boom_len: f32,
    boom_len_target: f32,
}

const MOVE_SPEED: f32 = 2.0;
const MOVE_SPEED_Y_SCALING: f32 = 0.3;
const ROTATE_SPEED: f32 = 0.01;
const MOVE_DAMPING_FACTOR: f32 = 0.001;
const ROTATE_DAMPING_FACTOR: f32 = 0.00001;
const ZOOM_DAMPING_FACTOR: f32 = 0.000001;

impl FrameData {
    pub fn new(static_mesh_interface: StaticMeshInterface) -> Self {
        Self {
            static_mesh_interface,
            origin: Default::default(),
            origin_vel: Default::default(),
            azimuth_angle: 0.0,
            azimuth_angle_target: 0.0,
            polar_angle: 0.5,
            polar_angle_target: 0.5,
            boom_len: 5.0,
            boom_len_target: 5.0,
        }
    }

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
                    self.boom_len_target -= delta * 0.01;
                    self.boom_len_target = self.boom_len_target.max(1.0).min(15.0);
                }
                _ => {}
            }
        }

        let rotate_alpha = 1.0 - ROTATE_DAMPING_FACTOR.powf(delta_time);
        self.azimuth_angle += (self.azimuth_angle_target - self.azimuth_angle) * rotate_alpha;
        self.polar_angle += (self.polar_angle_target - self.polar_angle) * rotate_alpha;

        let boom_len_alpha = 1.0 - ZOOM_DAMPING_FACTOR.powf(delta_time);
        self.boom_len += (self.boom_len_target - self.boom_len) * boom_len_alpha;

        let location = rotate_vec3(
            &vec3(0.0, 0.0, -self.boom_len),
            self.polar_angle,
            &vec3(1.0, 0.0, 0.0),
        );
        let location = rotate_vec3(&location, self.azimuth_angle, &vec3(0.0, 1.0, 0.0));

        let y_scaling = 1.0 + MOVE_SPEED_Y_SCALING * location.y;
        self.origin += self.origin_vel * MOVE_SPEED * y_scaling * delta_time;
        self.origin_vel *= MOVE_DAMPING_FACTOR.powf(delta_time);

        let location = self.origin + location;

        let orientation = (self.origin - location).normalize();

        // faux ensure camera isn't colliding
        self.static_mesh_interface
            .raycast(&self.origin, &-orientation)
            .await;

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
