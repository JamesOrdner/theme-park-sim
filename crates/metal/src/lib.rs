#![cfg(target_vendor = "apple")]

use frame_buffer::FrameBuffer;
use winit::window::Window;

pub struct Metal;

impl Metal {
    pub fn new(_window: &Window) -> Self {
        Self {}
    }

    pub async fn frame(&mut self, _frame_buffer: &FrameBuffer) {}
}
