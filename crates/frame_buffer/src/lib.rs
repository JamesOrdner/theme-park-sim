use std::{cell::Cell, marker::PhantomData, num::NonZeroUsize, ptr, sync::Arc};

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
    #[inline]
    pub fn spawned_static_meshes(&self) -> impl Iterator<Item = &SpawnedStaticMesh> {
        self.frame_buffer_manager.spawned_static_meshes.iter()
    }

    #[inline]
    pub fn despawned(&self) -> impl Iterator<Item = &EntityId> {
        self.frame_buffer_manager.despawned.iter()
    }

    #[inline]
    pub fn camera_info(&self) -> Option<CameraInfo> {
        let swap_index = self.frame_buffer_manager.read_index();
        self.frame_buffer_manager
            .event_buffers
            .iter()
            .find_map(|buffers| buffers[swap_index].camera_info)
    }

    #[inline]
    pub fn locations(&self) -> impl Iterator<Item = (EntityId, &Vec3)> {
        let swap_index = self.frame_buffer_manager.read_index();
        self.frame_buffer_manager
            .event_buffers
            .iter()
            .flat_map(move |buffers| &buffers[swap_index].locations)
            .map(|entity_data| (entity_data.entity_id, &entity_data.data))
    }
}

pub struct FrameBufferWriter<'a> {
    swap_index: bool,
    marker: PhantomData<&'a FrameBufferManager>,
}

impl FrameBufferWriter<'_> {
    #[inline]
    pub fn set_camera_info(&self, info: CameraInfo) {
        EVENT_BUFFER.with(|queue| unsafe {
            queue.get().as_mut().unwrap_unchecked()[self.swap_index as usize].camera_info =
                Some(info);
        });
    }

    #[inline]
    pub fn push_location(&self, entity_id: EntityId, location: Vec3) {
        EVENT_BUFFER.with(|queue| unsafe {
            queue.get().as_mut().unwrap_unchecked()[self.swap_index as usize]
                .locations
                .push(EntityData::new(entity_id, location));
        });
    }
}

pub struct SyncFrameBufferDelegate<'a> {
    frame_buffer_manager: &'a mut FrameBufferManager,
}

impl SyncFrameBufferDelegate<'_> {
    #[inline]
    pub fn spawn_static_mesh(&mut self, static_mesh: SpawnedStaticMesh) {
        self.frame_buffer_manager
            .spawned_static_meshes
            .push(static_mesh);
    }

    #[inline]
    pub fn despawn(&mut self, entity_id: EntityId) {
        self.frame_buffer_manager.despawned.push(entity_id);
    }
}

#[derive(Clone)]
pub struct SpawnedStaticMesh {
    pub entity_id: EntityId,
    pub resource: Arc<Resource>,
}

#[derive(Clone, Copy)]
pub struct CameraInfo {
    pub focus: Vec3,
    pub location: Vec3,
    pub up: Vec3,
}

#[derive(Clone, Default)]
struct Data {
    camera_info: Option<CameraInfo>,
    locations: Vec<EntityData<Vec3>>,
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

impl Data {
    fn clear(&mut self) {
        self.camera_info = None;
        self.locations.clear();
    }
}

pub struct FrameBufferManager {
    event_buffers: Vec<[Data; 2]>,
    spawned_static_meshes: Vec<SpawnedStaticMesh>,
    despawned: Vec<EntityId>,
    swap_index: bool,
}

impl FrameBufferManager {
    pub fn new(thread_count: NonZeroUsize) -> Self {
        Self {
            event_buffers: vec![[Data::default(), Data::default()]; thread_count.get()],
            spawned_static_meshes: Vec::new(),
            despawned: Vec::new(),
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
        self.despawned.clear();
    }

    fn read_index(&self) -> usize {
        !self.swap_index as usize
    }
}
