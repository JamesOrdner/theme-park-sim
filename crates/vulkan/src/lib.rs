#![cfg(not(target_vendor = "apple"))]

use frame_buffer::FrameBuffer;
use winit::window::Window;

pub struct Vulkan;

impl Vulkan {
    pub fn new(_window: &Window) -> Self {
        Self {}
    }

    pub async fn frame(&mut self, _frame_buffer: &FrameBuffer) {
        // artificial vsync
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}
