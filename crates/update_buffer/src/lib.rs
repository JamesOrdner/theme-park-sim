use std::{cell::Cell, num::NonZeroUsize, ptr::null_mut};

use game_entity::EntityId;
use nalgebra_glm::Vec3;

thread_local! {
    static UPDATE_BUFFER: Cell<*mut [Data; 2]> = Cell::new(null_mut())
}

#[derive(Default, Clone)]
struct Data {
    locations: Vec<EntityData<Vec3>>,
}

impl Data {
    fn clear(&mut self) {
        self.locations.clear();
    }
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

pub struct UpdateBuffer {
    update_buffers: Vec<[Data; 2]>,
    swap_index: bool,
}

impl UpdateBuffer {
    pub fn new(thread_count: NonZeroUsize) -> Self {
        Self {
            update_buffers: vec![Default::default(); thread_count.get()],
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

impl<'a> UpdateBufferRef<'a> {
    #[inline]
    pub fn locations(&self) -> impl Iterator<Item = (EntityId, &Vec3)> {
        let index = self.read_index();
        self.update_buffers
            .iter()
            .flat_map(move |buffers| &buffers[index].locations)
            .map(|entity_data| (entity_data.entity_id, &entity_data.data))
    }

    #[inline]
    pub fn push_location(&self, entity_id: EntityId, location: Vec3) {
        let index = self.write_index();

        UPDATE_BUFFER.with(|buffer| unsafe {
            buffer.get().as_mut().unwrap_unchecked()[index]
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
