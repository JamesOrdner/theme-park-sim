use std::{cell::Cell, num::NonZeroUsize, ptr};

use game_entity::EntityId;
use nalgebra_glm::{Vec2, Vec3};

#[derive(Clone, Copy)]
pub enum FrameEvent {
    CameraLocation(Vec3),
    CameraOrientation(Vec3),
    Location(EntityId),
}

#[derive(Clone, Copy)]
pub enum GameEvent {
    Spawn(EntityId),
    Despawn(EntityId),
}

#[derive(Clone, Copy)]
pub enum InputEvent {
    CameraMoveAxis(Vec2),
    CameraRotateAxis(Vec2),
    CameraZoom(f32),
    CursorMoved(Vec2),
    MouseButton(bool),
}

thread_local! {
    static FRAME_EVENT_BUFFER: Cell<*mut [Vec<FrameEvent>; 2]> = Cell::new(ptr::null_mut())
}

pub struct SyncEventDelegate<'a> {
    event_manager: &'a mut EventManager,
}

impl SyncEventDelegate<'_> {
    #[inline]
    pub fn push_game_event(&mut self, event: GameEvent) {
        self.event_manager.game_event_buffer.push(event);
    }

    #[inline]
    pub fn push_input_event(&mut self, event: InputEvent) {
        self.event_manager.input_event_buffer.push(event);
    }

    #[inline]
    pub fn input_events(&self) -> impl Iterator<Item = &InputEvent> {
        self.event_manager.input_event_buffer.iter()
    }

    /// Frame events which occurred in the previous frame
    #[inline]
    pub fn frame_events(&self) -> impl Iterator<Item = &FrameEvent> {
        let swap_index = self.event_manager.read_index();
        self.event_manager
            .event_buffers
            .iter()
            .flat_map(move |buffers| &buffers[swap_index])
    }
}

pub struct AsyncEventDelegate<'a> {
    event_manager: &'a EventManager,
}

impl AsyncEventDelegate<'_> {
    #[inline]
    pub fn push_frame_event(&self, event: FrameEvent) {
        let swap_index = self.event_manager.write_index();

        // SAFETY: no other access with this swap index aliases. This is guaranteed because
        // EventManager is exclusively borrowed as long as an EventDelegate exists, preventing
        // modification of the swap index or simultaneous access to the event buffers
        FRAME_EVENT_BUFFER.with(|queue| unsafe {
            queue.get().as_mut().unwrap_unchecked()[swap_index].push(event)
        });
    }

    #[inline]
    pub fn game_events(&self) -> impl Iterator<Item = &GameEvent> {
        self.event_manager.game_event_buffer.iter()
    }

    #[inline]
    pub fn input_events(&self) -> impl Iterator<Item = &InputEvent> {
        self.event_manager.input_event_buffer.iter()
    }

    /// Frame events which occurred in the previous frame
    #[inline]
    pub fn frame_events(&self) -> impl Iterator<Item = &FrameEvent> {
        let swap_index = self.event_manager.read_index();
        self.event_manager
            .event_buffers
            .iter()
            .flat_map(move |buffers| &buffers[swap_index])
    }
}

pub struct EventManager {
    event_buffers: Vec<[Vec<FrameEvent>; 2]>,
    game_event_buffer: Vec<GameEvent>,
    input_event_buffer: Vec<InputEvent>,
    swap_index: bool,
}

impl EventManager {
    pub fn new(thread_count: NonZeroUsize) -> Self {
        Self {
            event_buffers: vec![Default::default(); thread_count.get()],
            game_event_buffer: Vec::new(),
            input_event_buffer: Vec::new(),
            swap_index: false,
        }
    }

    pub fn assign_thread_event_buffer(&self, thread_index: usize) {
        FRAME_EVENT_BUFFER.with(|queue| queue.set(self.event_buffers[thread_index].as_ptr() as _));
    }

    pub fn sync_delegate(&mut self) -> SyncEventDelegate {
        SyncEventDelegate {
            event_manager: self,
        }
    }

    pub fn async_delegate(&mut self) -> AsyncEventDelegate {
        AsyncEventDelegate {
            event_manager: self,
        }
    }

    pub fn swap(&mut self) {
        self.swap_index = !self.swap_index;

        let index = self.write_index();
        for double_buffer in &mut self.event_buffers {
            double_buffer[index].clear();
        }

        self.game_event_buffer.clear();
        self.input_event_buffer.clear();
    }

    fn read_index(&self) -> usize {
        !self.swap_index as usize
    }

    fn write_index(&self) -> usize {
        self.swap_index as usize
    }
}
