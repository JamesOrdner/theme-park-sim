use event::{AsyncEventDelegate, InputEvent};
use frame_buffer::FrameBufferWriter;
use nalgebra_glm::Vec3;

#[derive(Default)]
pub struct FrameData {
    location: Vec3,
}

impl FrameData {
    pub async fn update(
        &mut self,
        event_delegate: &AsyncEventDelegate<'_>,
        frame_buffer: &FrameBufferWriter<'_>,
        delta_time: f32,
    ) {
        if let Some(input_move_axis) = event_delegate.input_events().find_map(|event| match event {
            InputEvent::CameraMoveAxis(axis) => Some(axis),
            _ => None,
        }) {
            self.location.x += input_move_axis.x * delta_time;
            self.location.y += input_move_axis.y * delta_time;
        }

        frame_buffer.set_camera_location(self.location);
    }
}
