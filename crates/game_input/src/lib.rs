use event::{EventDelegate, FrameEvent};
use nalgebra_glm::{vec2, Vec2};
use winit::{dpi::PhysicalSize, event::WindowEvent};

pub struct GameInputInterface<'a> {
    input: &'a GameInput,
}

impl<'a> GameInputInterface<'a> {
    pub fn new(input: &'a GameInput) -> Self {
        Self { input }
    }

    pub fn cursor_position(&self) -> &Vec2 {
        &self.input.cursor_position
    }
}

pub struct GameInput {
    window_size: Vec2,
    cursor_position: Vec2,
    cursor_moved: bool,
}

impl GameInput {
    pub fn new(window_size: PhysicalSize<u32>) -> Self {
        Self {
            window_size: vec2(window_size.width as f32, window_size.height as f32),
            cursor_position: Vec2::zeros(),
            cursor_moved: false,
        }
    }

    pub fn handle_input(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                self.window_size.x = size.width as f32;
                self.window_size.y = size.height as f32;
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position.x = position.x as f32;
                self.cursor_position.y = position.y as f32;
                self.cursor_moved = true;
            }
            _ => {}
        }
    }

    pub fn update(&mut self, event_delegate: &EventDelegate) {
        if self.cursor_moved {
            event_delegate.push_event(FrameEvent::CursorMoved);
            self.cursor_moved = false;
        }
    }
}
