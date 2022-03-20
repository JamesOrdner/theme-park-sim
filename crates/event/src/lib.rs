use std::{cell::Cell, marker::PhantomData, num::NonZeroUsize, ptr};

#[derive(Clone, Copy)]
pub enum FrameEvent {
    CursorMoved,
    Location(u32),
}

#[derive(Clone, Copy)]
pub enum FixedEvent {
    Location(u32),
}

thread_local! {
    static FRAME_EVENT_BUFFER: Cell<*mut [Vec<FrameEvent>; 2]> = Cell::new(ptr::null_mut())
}

thread_local! {
    static FIXED_EVENT_BUFFER: Cell<*mut [Vec<FixedEvent>; 2]> = Cell::new(ptr::null_mut())
}

/// Writer for new events
#[derive(Clone, Copy)]
pub struct EventWriter<'a> {
    swap_index: bool,
    // borrow ensures update index is not incremented while writer exists
    marker: PhantomData<&'a EventManager>,
}

impl EventWriter<'_> {
    pub fn push_event(&self, event: FrameEvent) {
        // SAFETY: no other borrow with this swap index aliases. This is guaranteed because
        // a &mut EventManager is required to modify the swap index, which is impossible
        // while an EventReader or EventWriter exists. It IS possible, however, for EventManager
        // to immutably access this buffer while we mutably access it, so TODO: combine the reader
        // and writer into one, and require a mutable borrow of EventManager to gain access to this
        // combo reader/writer. This then guarantees that EventManager cannot do anything simultaneously.
        FRAME_EVENT_BUFFER.with(|queue| unsafe {
            debug_assert!(!queue.get().is_null());
            queue.get().as_mut().unwrap_unchecked()[self.swap_index as usize].push(event)
        });
    }
}

/// Reader for past events
#[derive(Clone, Copy)]
pub struct EventReader<'a> {
    // borrow ensures update index is not incremented while reader exists
    marker: PhantomData<&'a EventManager>,
}

pub struct EventManager {
    event_buffers: Vec<[Vec<FrameEvent>; 2]>,
    swap_index: bool,
}

impl EventManager {
    pub fn new(thread_count: NonZeroUsize) -> Self {
        Self {
            event_buffers: vec![[Vec::new(), Vec::new()]; thread_count.get()],
            swap_index: false,
        }
    }

    pub fn assign_thread_event_buffer(&mut self, thread_index: usize) {
        FRAME_EVENT_BUFFER.with(|queue| queue.set(&mut self.event_buffers[thread_index]));
    }

    pub fn event_reader(&self) -> EventReader {
        EventReader {
            marker: PhantomData,
        }
    }

    pub fn event_writer(&self) -> EventWriter {
        EventWriter {
            swap_index: self.swap_index,
            marker: PhantomData,
        }
    }

    pub fn swap_buffers(&mut self) {
        self.swap_index = !self.swap_index;

        for event_buffer in &mut self.event_buffers {
            event_buffer[self.swap_index as usize].clear();
        }
    }
}
