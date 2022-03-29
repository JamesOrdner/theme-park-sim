use std::{cell::Cell, marker::PhantomData, num::NonZeroUsize, ptr};

use nalgebra_glm::Vec3;

pub struct FrameBufferDelegate<'a> {
    swap_index: bool,
    frame_buffer_manager: &'a FrameBufferManager,
}

impl<'a> FrameBufferDelegate<'a> {
    pub fn reader(&self) -> FrameBufferReader {
        FrameBufferReader {
            swap_index: !self.swap_index,
            frame_buffer_manager: self.frame_buffer_manager,
        }
    }

    pub fn writer(&self) -> FrameBufferWriter {
        FrameBufferWriter {
            swap_index: self.swap_index,
            marker: PhantomData,
        }
    }
}

pub struct FrameBufferReader<'a> {
    swap_index: bool,
    frame_buffer_manager: &'a FrameBufferManager,
}

impl FrameBufferReader<'_> {
    pub fn camera_location(&self) -> Option<Vec3> {
        self.frame_buffer_manager
            .event_buffers
            .iter()
            .find_map(|double_buffer| double_buffer[self.swap_index as usize].camera_location)
    }

    pub fn locations<F>(&self, f: F)
    where
        F: FnMut(&Vec3),
    {
        self.frame_buffer_manager
            .event_buffers
            .iter()
            .flat_map(|double_buffer| &double_buffer[self.swap_index as usize].locations)
            .for_each(f)
    }
}

pub struct FrameBufferWriter<'a> {
    swap_index: bool,
    marker: PhantomData<&'a FrameBufferManager>,
}

impl FrameBufferWriter<'_> {
    pub fn set_camera_location(&self, location: Vec3) {
        FRAME_BUFFER_ENTRY_BUFFER.with(|queue| unsafe {
            queue.get().as_mut().unwrap_unchecked()[self.swap_index as usize].camera_location =
                Some(location);
        });
    }

    pub fn push_location(&self, location: Vec3) {
        FRAME_BUFFER_ENTRY_BUFFER.with(|queue| unsafe {
            queue.get().as_mut().unwrap_unchecked()[self.swap_index as usize]
                .locations
                .push(location);
        });
    }
}

thread_local! {
    static FRAME_BUFFER_ENTRY_BUFFER: Cell<*mut [Data; 2]> = Cell::new(ptr::null_mut())
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
    swap_index: bool,
}

impl FrameBufferManager {
    pub fn new(thread_count: NonZeroUsize) -> Self {
        Self {
            event_buffers: vec![[Data::default(), Data::default()]; thread_count.get()],
            swap_index: false,
        }
    }

    pub fn assign_thread_frame_buffer(&self, thread_index: usize) {
        FRAME_BUFFER_ENTRY_BUFFER
            .with(|queue| queue.set(self.event_buffers[thread_index].as_ptr() as *mut _));
    }

    pub fn delegate(&mut self) -> FrameBufferDelegate {
        FrameBufferDelegate {
            swap_index: self.swap_index,
            frame_buffer_manager: self,
        }
    }

    pub fn swap(&mut self) {
        self.swap_index = !self.swap_index;

        for event_buffer in &mut self.event_buffers {
            event_buffer[self.swap_index as usize].clear();
        }
    }
}
