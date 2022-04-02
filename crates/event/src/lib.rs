use std::{cell::Cell, num::NonZeroUsize, ptr, slice::Iter};

use game_entity::EntityId;
use nalgebra_glm::Vec2;

#[derive(Clone, Copy)]
pub enum FrameEvent {
    Location(u32),
}

#[derive(Clone, Copy)]
pub enum GameEvent {
    Spawn(EntityId),
    Despawn(EntityId),
}

#[derive(Clone, Copy)]
pub enum InputEvent {
    CameraMoveAxis(Vec2),
    CursorMoved(Vec2),
    MouseButton(bool),
}

thread_local! {
    static FRAME_EVENT_BUFFER: Cell<*mut [Vec<FrameEvent>; 2]> = Cell::new(ptr::null_mut())
}

pub struct AsyncEventDelegate<'a> {
    event_manager: &'a EventManager,
}

impl AsyncEventDelegate<'_> {
    pub fn push_frame_event(&self, event: FrameEvent) {
        let swap_index = self.event_manager.write_index();

        // SAFETY: no other borrow with this swap index aliases. This is guaranteed because
        // EventManager is exclusively borrowed as long as an EventDelegate exists, preventing
        // modification of the swap index or simultaneous access to the event buffers
        FRAME_EVENT_BUFFER.with(|queue| unsafe {
            queue.get().as_mut().unwrap_unchecked()[swap_index].push(event)
        });
    }

    pub fn frame_events<F>(&self, f: F)
    where
        F: FnMut(&FrameEvent),
    {
        self.event_manager
            .event_buffers
            .iter()
            .flat_map(|double_buffer| &double_buffer[self.event_manager.read_index()])
            .for_each(f);
    }

    pub fn game_events(&self) -> Iter<GameEvent> {
        self.event_manager.game_event_buffers[self.event_manager.read_index()].iter()
    }

    pub fn input_events(&self) -> Iter<InputEvent> {
        self.event_manager.input_event_buffers[self.event_manager.read_index()].iter()
    }
}

pub struct SyncEventDelegate<'a> {
    event_manager: &'a mut EventManager,
}

impl SyncEventDelegate<'_> {
    pub fn push_frame_event(&mut self, event: FrameEvent) {
        let index = self.event_manager.write_index();
        self.event_manager.event_buffers[0][index].push(event);
    }

    pub fn push_game_event(&mut self, event: GameEvent) {
        let index = self.event_manager.write_index();
        self.event_manager.game_event_buffers[index].push(event);
    }

    pub fn push_input_event(&mut self, event: InputEvent) {
        let index = self.event_manager.write_index();
        self.event_manager.input_event_buffers[index].push(event);
    }

    pub fn frame_events<F>(&self, f: F)
    where
        F: FnMut(&FrameEvent),
    {
        self.event_manager
            .event_buffers
            .iter()
            .flat_map(|double_buffer| &double_buffer[self.event_manager.read_index()])
            .for_each(f);
    }

    pub fn game_events(&self) -> Iter<GameEvent> {
        self.event_manager.game_event_buffers[self.event_manager.read_index()].iter()
    }

    pub fn input_events(&self) -> Iter<InputEvent> {
        self.event_manager.input_event_buffers[self.event_manager.read_index()].iter()
    }
}

pub struct EventManager {
    event_buffers: Vec<[Vec<FrameEvent>; 2]>,
    game_event_buffers: [Vec<GameEvent>; 2],
    input_event_buffers: [Vec<InputEvent>; 2],
    swap_index: bool,
}

impl EventManager {
    pub fn new(thread_count: NonZeroUsize) -> Self {
        Self {
            event_buffers: vec![[Vec::new(), Vec::new()]; thread_count.get()],
            game_event_buffers: Default::default(),
            input_event_buffers: Default::default(),
            swap_index: false,
        }
    }

    pub fn assign_thread_event_buffer(&self, thread_index: usize) {
        FRAME_EVENT_BUFFER
            .with(|queue| queue.set(self.event_buffers[thread_index].as_ptr() as *mut _));
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

        self.game_event_buffers[index].clear();
        self.input_event_buffers[index].clear();
    }

    fn read_index(&self) -> usize {
        !self.swap_index as usize
    }

    fn write_index(&self) -> usize {
        self.swap_index as usize
    }
}
