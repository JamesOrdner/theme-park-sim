use erupt::vk;
use game_entity::EntityMap;
use gpu_alloc::UsageFlags;
use nalgebra_glm::Mat4;

use crate::allocator::{GpuAllocator, GpuBuffer};

pub struct Scene {
    pub static_meshes: EntityMap<StaticMesh>,
    pub delete_queue: Vec<GpuBuffer>,
    pub guests_buffer: GpuBuffer,
}

pub struct StaticMesh {
    pub vertex_buffer: GpuBuffer,
    pub vertex_offset: vk::DeviceSize,
    pub transform: Mat4,
}

impl Scene {
    pub fn new(allocator: &mut GpuAllocator) -> Self {
        let buffer_info = vk::BufferCreateInfoBuilder::new()
            .size(8 * 32) // TEMP
            .usage(vk::BufferUsageFlags::STORAGE_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let guests_buffer = allocator.alloc(&buffer_info, UsageFlags::FAST_DEVICE_ACCESS);

        Self {
            static_meshes: EntityMap::new(),
            delete_queue: Vec::new(),
            guests_buffer,
        }
    }

    pub unsafe fn destroy(self, allocator: &mut GpuAllocator) {
        for (_, static_mesh) in self.static_meshes {
            allocator.dealloc(static_mesh.vertex_buffer);
        }

        allocator.dealloc(self.guests_buffer);
    }
}
