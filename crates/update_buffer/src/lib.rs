use std::{marker::PhantomData, num::NonZeroUsize};

pub struct UpdateBuffer {
    swap_index: bool,
}

impl UpdateBuffer {
    pub fn new(_thread_count: NonZeroUsize) -> Self {
        Self { swap_index: false }
    }

    pub fn borrow(&self) -> UpdateBufferRef {
        UpdateBufferRef {
            marker: PhantomData,
        }
    }

    pub fn swap_buffers(&mut self) {
        self.swap_index = !self.swap_index;
    }
}

#[derive(Clone, Copy)]
pub struct UpdateBufferRef<'a> {
    marker: PhantomData<&'a UpdateBuffer>,
}

impl<'a> UpdateBufferRef<'a> {
    pub fn read(&self) {}
}
