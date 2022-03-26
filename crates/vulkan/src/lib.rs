#![cfg(not(target_vendor = "apple"))]

use frame_buffer::FrameBufferReader;
use winit::window::Window;

pub struct Vulkan;

impl Vulkan {
    pub fn new(_window: &Window) -> Self {
        Self {}
    }

    pub async fn frame(&mut self, frame_buffer: &FrameBufferReader<'_>) {
        // artificial vsync
        std::thread::sleep(std::time::Duration::from_millis(16));

        frame_buffer.locations(|location| println!("{}", location.x));
    }
}
