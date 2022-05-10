use erupt::vk;
use game_entity::EntityId;

use crate::allocator::{GpuAllocator, GpuBuffer};

#[derive(Default)]
pub struct Scene {
    pub static_meshes: Vec<StaticMesh>,
}

pub struct StaticMesh {
    pub entity_id: EntityId,
    pub vertex_buffer: GpuBuffer,
    pub vertex_offset: vk::DeviceSize,
}

impl Scene {
    pub unsafe fn destroy(&mut self, allocator: &mut GpuAllocator) {
        for static_mesh in self.static_meshes.drain(..) {
            allocator.dealloc(static_mesh.vertex_buffer);
        }
    }
}
