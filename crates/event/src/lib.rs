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

#[derive(Clone, Copy)]
pub struct EventDelegate<'a> {
    swap_index: bool,
    // borrow ensures update index is not incremented while writer exists
    marker: PhantomData<&'a mut EventManager>,
}

impl EventDelegate<'_> {
    pub fn push_event(&self, event: FrameEvent) {
        // SAFETY: no other borrow with this swap index aliases. This is guaranteed because
        // EventManager is mutably borrowed as long as an EventDelegate exists, preventing
        // modification of the swap index or simultaneous access to the event buffers
        FRAME_EVENT_BUFFER.with(|queue| unsafe {
            debug_assert!(!queue.get().is_null());
            queue.get().as_mut().unwrap_unchecked()[self.swap_index as usize].push(event)
        });
    }
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

    pub fn borrow(&mut self) -> EventDelegate {
        EventDelegate {
            swap_index: self.swap_index,
            marker: PhantomData,
        }
    }

    pub fn swap(&mut self) {
        self.swap_index = !self.swap_index;

        for event_buffer in &mut self.event_buffers {
            event_buffer[self.swap_index as usize].clear();
        }
    }
}
