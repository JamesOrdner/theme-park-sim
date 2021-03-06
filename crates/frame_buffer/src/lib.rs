use std::{cell::Cell, marker::PhantomData, num::NonZeroUsize, ptr, sync::Arc};

use game_entity::EntityId;
use game_resources::Resource;
use nalgebra_glm::Vec3;

thread_local! {
    static EVENT_BUFFER: Cell<*mut [Data; 2]> = Cell::new(ptr::null_mut())
}

pub struct AsyncFrameBufferDelegate<'a> {
    inner: &'a FrameBufferManager,
}

impl<'a> AsyncFrameBufferDelegate<'a> {
    #[inline]
    pub fn reader(&self) -> FrameBufferReader {
        FrameBufferReader { inner: self.inner }
    }

    #[inline]
    pub fn writer(&self) -> FrameBufferWriter {
        FrameBufferWriter {
            swap_index: self.inner.swap_index,
            marker: PhantomData,
        }
    }
}

pub struct FrameBufferReader<'a> {
    inner: &'a FrameBufferManager,
}

impl FrameBufferReader<'_> {
    #[inline]
    pub fn spawned_guests(&self) -> impl Iterator<Item = &EntityId> {
        self.inner.spawned_guests.iter()
    }

    #[inline]
    pub fn spawned_static_meshes(&self) -> impl Iterator<Item = &SpawnedStaticMesh> {
        self.inner.spawned_static_meshes.iter()
    }

    #[inline]
    pub fn despawned(&self) -> impl Iterator<Item = &EntityId> {
        self.inner.despawned.iter()
    }

    #[inline]
    pub fn updated_entity_ids(&self) -> impl Iterator<Item = &(EntityId, EntityId)> {
        self.inner.updated_entity_ids.iter()
    }

    #[inline]
    pub fn camera_info(&self) -> &CameraInfo {
        &self.inner.camera_info
    }

    #[inline]
    pub fn locations(&self) -> impl Iterator<Item = (EntityId, &Vec3)> {
        let swap_index = self.inner.read_index();
        self.inner
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
    pub fn push_location(&self, entity_id: EntityId, location: Vec3) {
        EVENT_BUFFER.with(|queue| unsafe {
            queue.get().as_mut().unwrap_unchecked()[self.swap_index as usize]
                .locations
                .push(EntityData::new(entity_id, location));
        });
    }
}

pub struct SyncFrameBufferDelegate<'a> {
    inner: &'a mut FrameBufferManager,
}

impl SyncFrameBufferDelegate<'_> {
    #[inline]
    pub fn spawn_guest(&mut self, entity_id: EntityId) {
        self.inner.spawned_guests.push(entity_id);
    }

    #[inline]
    pub fn spawn_static_mesh(&mut self, static_mesh: SpawnedStaticMesh) {
        self.inner.spawned_static_meshes.push(static_mesh);
    }

    #[inline]
    pub fn despawn(&mut self, entity_id: EntityId) {
        self.inner.despawned.push(entity_id);
    }

    #[inline]
    pub fn update_entity_id(&mut self, old_id: EntityId, new_id: EntityId) {
        self.inner.updated_entity_ids.push((old_id, new_id));
    }

    #[inline]
    pub fn set_camera_info(&mut self, info: CameraInfo) {
        self.inner.camera_info = info;
    }

    #[inline]
    pub fn push_location(&mut self, entity_id: EntityId, location: Vec3) {
        let index = self.inner.read_index(); // post-swap
        self.inner.event_buffers[0][index]
            .locations
            .push(EntityData::new(entity_id, location));
    }
}

#[derive(Clone)]
pub struct SpawnedStaticMesh {
    pub entity_id: EntityId,
    pub resource: Arc<Resource>,
}

pub struct CameraInfo {
    pub focus: Vec3,
    pub location: Vec3,
    pub up: Vec3,
    pub fov: f32,
    pub near_plane: f32,
    pub far_plane: f32,
}

impl Default for CameraInfo {
    fn default() -> Self {
        // some reasonable default but actual values don't matter
        Self {
            focus: Vec3::zeros(),
            location: Vec3::from([0.0, 0.0, 1.0]),
            up: Vec3::from([0.0, 1.0, 0.0]),
            fov: 1.0,
            near_plane: 0.01,
            far_plane: 50.0,
        }
    }
}

#[derive(Clone, Default)]
struct Data {
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
        self.locations.clear();
    }
}

pub struct FrameBufferManager {
    event_buffers: Vec<[Data; 2]>,
    spawned_guests: Vec<EntityId>,
    spawned_static_meshes: Vec<SpawnedStaticMesh>,
    despawned: Vec<EntityId>,
    updated_entity_ids: Vec<(EntityId, EntityId)>,
    camera_info: CameraInfo,
    swap_index: bool,
}

impl FrameBufferManager {
    pub fn new(thread_count: NonZeroUsize) -> Self {
        Self {
            event_buffers: vec![[Data::default(), Data::default()]; thread_count.get()],
            spawned_guests: Vec::new(),
            spawned_static_meshes: Vec::new(),
            despawned: Vec::new(),
            updated_entity_ids: Vec::new(),
            camera_info: CameraInfo::default(),
            swap_index: false,
        }
    }

    pub fn assign_thread_frame_buffer(&self, thread_index: usize) {
        EVENT_BUFFER.with(|queue| queue.set(self.event_buffers[thread_index].as_ptr() as *mut _));
    }

    pub fn sync_delegate(&mut self) -> SyncFrameBufferDelegate {
        SyncFrameBufferDelegate { inner: self }
    }

    pub fn async_delegate(&mut self) -> AsyncFrameBufferDelegate {
        AsyncFrameBufferDelegate { inner: self }
    }

    pub fn swap(&mut self) {
        self.swap_index = !self.swap_index;

        for event_buffer in &mut self.event_buffers {
            event_buffer[self.swap_index as usize].clear();
        }

        self.spawned_guests.clear();
        self.spawned_static_meshes.clear();
        self.despawned.clear();
        self.updated_entity_ids.clear();
    }

    fn read_index(&self) -> usize {
        !self.swap_index as usize
    }
}
