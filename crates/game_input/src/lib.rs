use std::ops::{Deref, DerefMut};

use event::{InputEvent, SyncEventDelegate};
use nalgebra_glm::{vec2, Vec2};
use winit::{
    dpi::PhysicalSize,
    event::{
        DeviceEvent, ElementState, MouseButton, MouseScrollDelta, VirtualKeyCode, WindowEvent,
    },
};

#[derive(Clone, Copy)]
pub struct GameInputInterface<'a> {
    inner: &'a GameInput,
}

impl<'a> GameInputInterface<'a> {
    #[inline]
    pub fn cursor_position(&self) -> &Vec2 {
        &*self.inner.cursor_position
    }

    #[inline]
    pub fn cursor_position_ndc(&self) -> Vec2 {
        Vec2::from([
            self.inner.cursor_position.x * 2.0 / self.inner.window_size.x - 1.0,
            self.inner.cursor_position.y * 2.0 / self.inner.window_size.y - 1.0,
        ])
    }
}

struct InputState<T> {
    val: T,
    modified: bool,
}

impl<T> Default for InputState<T>
where
    T: Default,
{
    fn default() -> Self {
        Self {
            val: Default::default(),
            modified: false,
        }
    }
}

impl<T> InputState<T> {
    fn updated(&mut self) -> Option<&T> {
        if self.modified {
            self.modified = false;
            Some(&self.val)
        } else {
            None
        }
    }
}

impl<T> Deref for InputState<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.val
    }
}

impl<T> DerefMut for InputState<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.modified = true;
        &mut self.val
    }
}

pub struct GameInput {
    window_size: Vec2,
    cursor_position: InputState<Vec2>,
    left_mouse_button: InputState<bool>,
    camera_movement: Vec2,
    camera_rotating: bool,
    camera_rotation: Vec2,
    camera_zoom: f32,
}

impl GameInput {
    pub fn new(window_size: PhysicalSize<u32>) -> Self {
        Self {
            window_size: vec2(window_size.width as f32, window_size.height as f32),
            cursor_position: Default::default(),
            left_mouse_button: Default::default(),
            camera_movement: Default::default(),
            camera_rotating: false,
            camera_rotation: Default::default(),
            camera_zoom: Default::default(),
        }
    }

    pub fn interface(&self) -> GameInputInterface {
        GameInputInterface { inner: self }
    }

    pub fn handle_raw_input(&mut self, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.camera_rotating {
                self.camera_rotation.x += delta.0 as f32;
                self.camera_rotation.y += delta.1 as f32;
            }
        }
    }

    pub fn handle_input(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position.x = position.x as f32;
                self.cursor_position.y = position.y as f32;
            }
            WindowEvent::KeyboardInput { input, .. } => {
                let pressed = input.state == ElementState::Pressed;
                if let Some(keycode) = input.virtual_keycode {
                    match keycode {
                        VirtualKeyCode::W => {
                            self.camera_movement.y = if pressed { 1.0 } else { 0.0 };
                        }
                        VirtualKeyCode::A => {
                            self.camera_movement.x = if pressed { -1.0 } else { 0.0 };
                        }
                        VirtualKeyCode::S => {
                            self.camera_movement.y = if pressed { -1.0 } else { 0.0 };
                        }
                        VirtualKeyCode::D => {
                            self.camera_movement.x = if pressed { 1.0 } else { 0.0 };
                        }
                        VirtualKeyCode::Space => {
                            self.camera_rotating = pressed;
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::MouseInput { button, state, .. } => {
                let pressed = state == ElementState::Pressed;
                match button {
                    MouseButton::Left => {
                        *self.left_mouse_button = pressed;
                    }
                    MouseButton::Middle => {
                        self.camera_rotating = pressed;
                    }
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.camera_zoom += match delta {
                    MouseScrollDelta::LineDelta(_, lines) => lines * 50.0,
                    MouseScrollDelta::PixelDelta(pixels) => pixels.y as f32,
                };
            }
            WindowEvent::Resized(size) => {
                self.window_size.x = size.width as f32;
                self.window_size.y = size.height as f32;
            }
            _ => {}
        }
    }

    pub fn update(&mut self, event_delegate: &mut SyncEventDelegate) {
        if self.cursor_position.updated().is_some() {
            event_delegate.push_input_event(InputEvent::CursorMoved);
        }

        if let Some(left_mouse_button) = self.left_mouse_button.updated() {
            event_delegate.push_input_event(InputEvent::MouseButton(*left_mouse_button));
        }

        // axis events are updated every frame

        event_delegate.push_input_event(InputEvent::CameraMoveAxis(self.camera_movement));
        event_delegate.push_input_event(InputEvent::CameraRotateAxis(self.camera_rotation));
        event_delegate.push_input_event(InputEvent::CameraZoom(self.camera_zoom));

        self.camera_rotation = Vec2::zeros();
        self.camera_zoom = 0.0;
    }
}
