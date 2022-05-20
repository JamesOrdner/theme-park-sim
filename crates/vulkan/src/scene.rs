use erupt::vk;
use game_entity::EntityMap;

use crate::allocator::{GpuAllocator, GpuBuffer};

#[derive(Default)]
pub struct Scene {
    pub static_meshes: EntityMap<StaticMesh>,
    pub delete_queue: Vec<GpuBuffer>,
}

pub struct StaticMesh {
    pub vertex_buffer: GpuBuffer,
    pub vertex_offset: vk::DeviceSize,
}

impl Scene {
    pub unsafe fn destroy(&mut self, allocator: &mut GpuAllocator) {
        for (_, static_mesh) in self.static_meshes.drain(..) {
            allocator.dealloc(static_mesh.vertex_buffer);
        }
    }
}
