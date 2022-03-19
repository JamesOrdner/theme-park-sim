#![cfg(not(target_vendor = "apple"))]

use frame_buffer::FrameBuffer;
use winit::window::Window;

pub struct Vulkan;

impl Vulkan {
    pub fn new(_window: &Window) -> Self {
        Self {}
    }

    pub fn frame(&mut self, _frame_buffer: &FrameBuffer) {}
}
