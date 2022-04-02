use std::{cell::Cell, marker::PhantomData, num::NonZeroUsize, ptr, slice::Iter, sync::Arc};

use game_entity::EntityId;
use game_resources::Resource;
use nalgebra_glm::Vec3;

thread_local! {
    static EVENT_BUFFER: Cell<*mut [Data; 2]> = Cell::new(ptr::null_mut())
}

pub struct AsyncFrameBufferDelegate<'a> {
    frame_buffer_manager: &'a FrameBufferManager,
}

impl<'a> AsyncFrameBufferDelegate<'a> {
    pub fn reader(&self) -> FrameBufferReader {
        FrameBufferReader {
            frame_buffer_manager: self.frame_buffer_manager,
        }
    }

    pub fn writer(&self) -> FrameBufferWriter {
        FrameBufferWriter {
            swap_index: self.frame_buffer_manager.swap_index,
            marker: PhantomData,
        }
    }
}

pub struct FrameBufferReader<'a> {
    frame_buffer_manager: &'a FrameBufferManager,
}

impl FrameBufferReader<'_> {
    pub fn spawned_static_meshes(&self) -> Iter<SpawnedStaticMesh> {
        self.frame_buffer_manager.spawned_static_meshes.iter()
    }

    pub fn camera_location(&self) -> Option<Vec3> {
        let swap_index = self.frame_buffer_manager.read_index();
        self.frame_buffer_manager
            .event_buffers
            .iter()
            .find_map(|buffers| buffers[swap_index].camera_location)
    }

    pub fn locations<F>(&self, f: F)
    where
        F: FnMut(&Vec3),
    {
        let swap_index = self.frame_buffer_manager.read_index();
        self.frame_buffer_manager
            .event_buffers
            .iter()
            .flat_map(|buffers| &buffers[swap_index].locations)
            .for_each(f)
    }
}

pub struct FrameBufferWriter<'a> {
    swap_index: bool,
    marker: PhantomData<&'a FrameBufferManager>,
}

impl FrameBufferWriter<'_> {
    pub fn set_camera_location(&self, location: Vec3) {
        EVENT_BUFFER.with(|queue| unsafe {
            queue.get().as_mut().unwrap_unchecked()[self.swap_index as usize].camera_location =
                Some(location);
        });
    }

    pub fn push_location(&self, location: Vec3) {
        EVENT_BUFFER.with(|queue| unsafe {
            queue.get().as_mut().unwrap_unchecked()[self.swap_index as usize]
                .locations
                .push(location);
        });
    }
}

pub struct SyncFrameBufferDelegate<'a> {
    frame_buffer_manager: &'a mut FrameBufferManager,
}

impl SyncFrameBufferDelegate<'_> {
    pub fn spawn_static_mesh(&mut self, static_mesh: SpawnedStaticMesh) {
        self.frame_buffer_manager
            .spawned_static_meshes
            .push(static_mesh);
    }
}

#[derive(Clone)]
pub struct SpawnedStaticMesh {
    pub id: EntityId,
    pub resource: Arc<Resource>,
}

#[derive(Clone, Default)]
struct Data {
    camera_location: Option<Vec3>,
    locations: Vec<Vec3>,
}

impl Data {
    fn clear(&mut self) {
        self.camera_location = None;
        self.locations.clear();
    }
}

pub struct FrameBufferManager {
    event_buffers: Vec<[Data; 2]>,
    spawned_static_meshes: Vec<SpawnedStaticMesh>,
    swap_index: bool,
}

impl FrameBufferManager {
    pub fn new(thread_count: NonZeroUsize) -> Self {
        Self {
            event_buffers: vec![[Data::default(), Data::default()]; thread_count.get()],
            spawned_static_meshes: Vec::new(),
            swap_index: false,
        }
    }

    pub fn assign_thread_frame_buffer(&self, thread_index: usize) {
        EVENT_BUFFER.with(|queue| queue.set(self.event_buffers[thread_index].as_ptr() as *mut _));
    }

    pub fn sync_delegate(&mut self) -> SyncFrameBufferDelegate {
        SyncFrameBufferDelegate {
            frame_buffer_manager: self,
        }
    }

    pub fn async_delegate(&mut self) -> AsyncFrameBufferDelegate {
        AsyncFrameBufferDelegate {
            frame_buffer_manager: self,
        }
    }

    pub fn swap(&mut self) {
        self.swap_index = !self.swap_index;

        for event_buffer in &mut self.event_buffers {
            event_buffer[self.swap_index as usize].clear();
        }

        self.spawned_static_meshes.clear();
    }

    fn read_index(&self) -> usize {
        !self.swap_index as usize
    }
}
