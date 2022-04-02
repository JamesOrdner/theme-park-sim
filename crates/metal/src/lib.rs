#![cfg(target_vendor = "apple")]

use frame_buffer::FrameBufferReader;
use winit::window::Window;

pub struct Metal;

impl Metal {
    pub fn new(_window: &Window) -> Self {
        Self {}
    }

    pub async fn frame(&mut self, frame_buffer: &FrameBufferReader<'_>) {
        // artificial vsync
        std::thread::sleep(std::time::Duration::from_millis(16));

        for static_mesh in frame_buffer.spawned_static_meshes() {
            let _ = static_mesh.resource.render_mesh();
        }
    }
}
