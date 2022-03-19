use event::{Event, EventWriter};
use nalgebra_glm::Vec2;
use winit::event::WindowEvent;

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

#[derive(Default)]
pub struct GameInput {
    cursor_position: Vec2,
}

impl GameInput {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn handle_input(&mut self, _event: WindowEvent) {}

    pub fn update(&mut self, event_writer: EventWriter) {
        event_writer.push_event(Event::CursorMoved);
    }
}
