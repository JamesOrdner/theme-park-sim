use std::ops::{Deref, DerefMut};

use event::{InputEvent, SyncEventDelegate};
use nalgebra_glm::{vec2, Vec2};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, MouseButton, WindowEvent},
};

pub struct GameInputInterface<'a> {
    input: &'a GameInput,
}

impl<'a> GameInputInterface<'a> {
    pub fn new(input: &'a GameInput) -> Self {
        Self { input }
    }

    pub fn cursor_position(&self) -> &Vec2 {
        &*self.input.cursor_position
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
}

impl GameInput {
    pub fn new(window_size: PhysicalSize<u32>) -> Self {
        Self {
            window_size: vec2(window_size.width as f32, window_size.height as f32),
            cursor_position: Default::default(),
            left_mouse_button: Default::default(),
        }
    }

    pub fn handle_input(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position.x = position.x as f32;
                self.cursor_position.y = position.y as f32;
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                *self.left_mouse_button = state == ElementState::Pressed;
            }
            WindowEvent::Resized(size) => {
                self.window_size.x = size.width as f32;
                self.window_size.y = size.height as f32;
            }
            _ => {}
        }
    }

    pub fn update(&mut self, event_delegate: &mut SyncEventDelegate) {
        if let Some(cursor_position) = self.cursor_position.updated() {
            event_delegate.push_input_event(InputEvent::CursorMoved(*cursor_position));
        }

        if let Some(left_mouse_button) = self.left_mouse_button.updated() {
            event_delegate.push_input_event(InputEvent::MouseButton(*left_mouse_button));
        }
    }
}
