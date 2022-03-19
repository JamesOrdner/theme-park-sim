use std::{cell::Cell, marker::PhantomData, num::NonZeroUsize, ptr};

#[derive(Clone, Copy)]
pub enum Event {
    CursorMoved,
}

thread_local! {
    static EVENT_BUFFER: Cell<*mut [Vec<Event>; 2]> = Cell::new(ptr::null_mut())
}

/// Writer for new events
#[derive(Clone, Copy)]
pub struct EventWriter<'a> {
    swap_index: bool,
    // borrow ensures update index is not incremented while writer exists
    marker: PhantomData<&'a EventManager>,
}

impl EventWriter<'_> {
    pub fn push_event(&self, event: Event) {
        EVENT_BUFFER.with(|queue| unsafe {
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
    event_buffers: Vec<[Vec<Event>; 2]>,
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
        EVENT_BUFFER.with(|queue| queue.set(&mut self.event_buffers[thread_index]));
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

    pub fn step(&mut self) {
        self.swap_index = !self.swap_index;

        for event_buffer in &mut self.event_buffers {
            event_buffer[self.swap_index as usize].clear();
        }
    }
}
