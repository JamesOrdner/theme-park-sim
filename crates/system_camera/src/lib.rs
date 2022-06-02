use std::f32::consts::FRAC_PI_2;

use event::{InputEvent, SyncEventDelegate};
use frame_buffer::{CameraInfo, SyncFrameBufferDelegate};
use nalgebra_glm::{inverse, look_at, perspective, rotate_vec3, vec3, vec4, Vec2, Vec3};
use system_interfaces::physics::Interface as PhysicsInterface;

const NEAR_PLANE: f32 = 0.01;
const FAR_PLANE: f32 = 50.0;

#[derive(Clone, Copy)]
pub struct CameraInterface<'a> {
    inner: &'a FrameData,
}

impl<'a> CameraInterface<'a> {
    pub fn location(&self) -> &Vec3 {
        &self.inner.location
    }

    /// Returns orientation
    pub fn deproject(&self, ndc: &Vec2) -> Vec3 {
        let orientation = (self.inner.origin - self.inner.location).normalize();
        let proj = perspective(self.inner.aspect, 1.0, NEAR_PLANE, FAR_PLANE);
        let view = look_at(&Vec3::zeros(), &orientation, &vec3(0.0, 1.0, 0.0));
        let vp_inv = inverse(&(proj * view));
        let screen = vec4(-ndc.x, -ndc.y, 1.0, 1.0);

        (vp_inv * screen).xyz().normalize()
    }
}

pub struct FrameData {
    physics: PhysicsInterface,
    aspect: f32,
    fov: f32,
    location: Vec3,
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
    pub fn new(window_width: u32, window_height: u32, physics: PhysicsInterface) -> Self {
        let aspect = window_width as f32 / window_height as f32;

        Self {
            physics,
            aspect,
            fov: 1.0,
            location: Default::default(),
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

    pub fn interface(&self) -> CameraInterface {
        CameraInterface { inner: self }
    }

    pub fn window_resized(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }

    pub fn update(
        &mut self,
        event_delegate: &SyncEventDelegate,
        frame_buffer: &mut SyncFrameBufferDelegate,
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

        let mut location = self.origin + location;
        let orientation = (self.origin - location).normalize();

        // camera collision
        if let Some(hit_location) = self.physics.raycast(&self.origin, &-orientation) {
            location = hit_location;
        }

        let camera_info = CameraInfo {
            focus: self.origin,
            location,
            up: vec3(0.0, 1.0, 0.0),
            fov: self.fov,
            near_plane: NEAR_PLANE,
            far_plane: FAR_PLANE,
        };

        frame_buffer.set_camera_info(camera_info);

        self.location = location;
    }
}
