use std::{cell::Cell, num::NonZeroUsize, ptr::null_mut};

use game_entity::EntityId;
use nalgebra_glm::Vec3;

#[derive(Clone, Copy)]
pub enum System {
    Network,
    StaticMesh,
}

thread_local! {
    static UPDATE_BUFFER: Cell<*mut [Data; 2]> = Cell::new(null_mut())
}

#[derive(Clone, Copy)]
pub struct EntityData<T> {
    pub entity_id: EntityId,
    pub data: T,
}

impl<T> EntityData<T> {
    fn new(entity_id: EntityId, data: T) -> Self {
        Self { entity_id, data }
    }
}

#[derive(Default)]
struct Data {
    network: Network,
    static_mesh: StaticMesh,
}

impl Data {
    fn clear(&mut self) {
        self.network.clear();
        self.static_mesh.clear();
    }
}

/// Updates that the network system will read.
#[derive(Default)]
struct Network {
    locations: Vec<EntityData<Vec3>>,
    spawned: Vec<EntityId>,
}

impl Network {
    fn clear(&mut self) {
        self.locations.clear();
        self.spawned.clear();
    }
}

/// Updates that the static mesh system will read.
#[derive(Default)]
struct StaticMesh {
    locations: Vec<EntityData<Vec3>>,
}

impl StaticMesh {
    fn clear(&mut self) {
        self.locations.clear();
    }
}

pub struct UpdateBuffer {
    update_buffers: Vec<[Data; 2]>,
    swap_index: bool,
}

impl UpdateBuffer {
    pub fn new(thread_count: NonZeroUsize) -> Self {
        let mut update_buffers = Vec::with_capacity(thread_count.get());
        for _ in 0..thread_count.get() {
            update_buffers.push(Default::default());
        }

        Self {
            update_buffers,
            swap_index: false,
        }
    }

    pub fn assign_thread_event_buffer(&self, thread_index: usize) {
        UPDATE_BUFFER.with(|queue| queue.set(self.update_buffers[thread_index].as_ptr() as _));
    }

    pub fn borrow(&mut self) -> UpdateBufferRef {
        UpdateBufferRef {
            update_buffers: &self.update_buffers,
            swap_index: self.swap_index,
        }
    }

    pub fn swap_buffers(&mut self) {
        self.swap_index = !self.swap_index;

        let index = self.swap_index as usize;
        for buffer in &mut self.update_buffers {
            buffer[index].clear();
        }
    }
}

#[derive(Clone, Copy)]
pub struct UpdateBufferRef<'a> {
    update_buffers: &'a Vec<[Data; 2]>,
    swap_index: bool,
}

impl UpdateBufferRef<'_> {
    pub fn network(&self) -> NetworkUpdateBufferRef {
        NetworkUpdateBufferRef {
            update_buffers: self.update_buffers,
            swap_index: self.swap_index,
        }
    }

    pub fn static_mesh(&self) -> StaticMeshUpdateBufferRef {
        StaticMeshUpdateBufferRef {
            update_buffers: self.update_buffers,
            swap_index: self.swap_index,
        }
    }
}

#[derive(Clone, Copy)]
pub struct NetworkUpdateBufferRef<'a> {
    update_buffers: &'a Vec<[Data; 2]>,
    swap_index: bool,
}

impl<'a> NetworkUpdateBufferRef<'a> {
    #[inline]
    pub fn locations(&self) -> impl Iterator<Item = (EntityId, &Vec3)> {
        let index = self.read_index();
        self.update_buffers
            .iter()
            .flat_map(move |buffers| &buffers[index].network.locations)
            .map(|entity_data| (entity_data.entity_id, &entity_data.data))
    }

    #[inline]
    pub fn spawned(&self) -> impl Iterator<Item = &EntityId> {
        let index = self.read_index();
        self.update_buffers
            .iter()
            .flat_map(move |buffers| &buffers[index].network.spawned)
    }

    #[inline]
    pub fn push_location(&self, entity_id: EntityId, location: Vec3) {
        let index = self.write_index();

        UPDATE_BUFFER.with(|buffer| unsafe {
            let buffer = &mut buffer.get().as_mut().unwrap_unchecked()[index];

            buffer
                .static_mesh
                .locations
                .push(EntityData::new(entity_id, location))
        });
    }

    fn read_index(&self) -> usize {
        !self.swap_index as usize
    }

    fn write_index(&self) -> usize {
        self.swap_index as usize
    }
}

#[derive(Clone, Copy)]
pub struct StaticMeshUpdateBufferRef<'a> {
    update_buffers: &'a Vec<[Data; 2]>,
    swap_index: bool,
}

impl<'a> StaticMeshUpdateBufferRef<'a> {
    #[inline]
    pub fn locations(&self) -> impl Iterator<Item = (EntityId, &Vec3)> {
        let index = self.read_index();
        self.update_buffers
            .iter()
            .flat_map(move |buffers| &buffers[index].static_mesh.locations)
            .map(|entity_data| (entity_data.entity_id, &entity_data.data))
    }

    #[inline]
    pub fn push_location(&self, entity_id: EntityId, location: Vec3) {
        let index = self.write_index();

        UPDATE_BUFFER.with(|buffer| unsafe {
            let buffer = &mut buffer.get().as_mut().unwrap_unchecked()[index];

            buffer
                .network
                .locations
                .push(EntityData::new(entity_id, location))
        });
    }

    #[inline]
    pub fn push_spawn(&self, entity_id: EntityId) {
        let index = self.write_index();

        UPDATE_BUFFER.with(|buffer| unsafe {
            let buffer = &mut buffer.get().as_mut().unwrap_unchecked()[index];
            buffer.network.spawned.push(entity_id)
        });
    }

    fn read_index(&self) -> usize {
        !self.swap_index as usize
    }

    fn write_index(&self) -> usize {
        self.swap_index as usize
    }
}
