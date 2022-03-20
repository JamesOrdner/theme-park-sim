use std::num::NonZeroUsize;

#[cfg(debug_assertions)]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct UpdateBuffer {
    swap_index: bool,

    #[cfg(debug_assertions)]
    ref_invalid: Arc<AtomicBool>,
}

impl UpdateBuffer {
    pub fn new(_thread_count: NonZeroUsize) -> Self {
        Self {
            swap_index: false,

            #[cfg(debug_assertions)]
            ref_invalid: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn borrow(&self) -> UpdateBufferRef {
        UpdateBufferRef {
            #[cfg(debug_assertions)]
            ref_invalid: self.ref_invalid.clone(),
        }
    }

    pub fn swap_buffers(&mut self) {
        self.swap_index = !self.swap_index;

        #[cfg(debug_assertions)]
        {
            self.ref_invalid.store(true, Ordering::Release);
            self.ref_invalid = Arc::new(AtomicBool::new(false));
        }
    }
}

#[derive(Clone, Default)]
pub struct UpdateBufferRef {
    #[cfg(debug_assertions)]
    ref_invalid: Arc<AtomicBool>,
}

impl UpdateBufferRef {
    pub fn read(&self) {
        #[cfg(debug_assertions)]
        assert!(!self.ref_invalid.load(Ordering::Acquire));
    }
}
