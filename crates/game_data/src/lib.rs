use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use async_lock::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub struct SharedData<T0 = (), T1 = (), const T1_LEN: usize = 0> {
    inner: Arc<SharedDataImpl<T0, T1, T1_LEN>>,
}

struct SharedDataImpl<T0 = (), T1 = (), const T1_LEN: usize = 0> {
    single_data: RwLock<T0>,
    multiple_data: [RwLock<T1>; T1_LEN],
    swap_index: AtomicUsize,
}

impl<T0, T1, const T1_LEN: usize> Clone for SharedData<T0, T1, T1_LEN> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T0> SharedData<T0, (), 0> {
    pub fn new_single(data: T0) -> Self {
        let inner = Arc::new(SharedDataImpl {
            single_data: RwLock::new(data),
            multiple_data: [],
            swap_index: AtomicUsize::new(0),
        });

        Self { inner }
    }
}

impl<T0, T1, const T1_LEN: usize> Default for SharedData<T0, T1, T1_LEN>
where
    T0: Default,
    T1: Default,
{
    fn default() -> Self {
        let inner = Arc::new(SharedDataImpl {
            single_data: RwLock::new(Default::default()),
            multiple_data: [0; T1_LEN].map(|_| RwLock::new(Default::default())),
            swap_index: AtomicUsize::new(0),
        });

        Self { inner }
    }
}

impl<T1, const T1_LEN: usize> SharedData<(), T1, T1_LEN>
where
    T1: Copy,
{
    pub fn new_multiple(data: T1) -> Self {
        let inner = Arc::new(SharedDataImpl {
            single_data: RwLock::new(()),
            multiple_data: [0; T1_LEN].map(|_| RwLock::new(data)),
            swap_index: AtomicUsize::new(0),
        });

        Self { inner }
    }
}

impl<T1, const T1_LEN: usize> SharedData<(), T1, T1_LEN>
where
    T1: Clone,
{
    pub fn new_multiple_clone(data: T1) -> Self {
        let inner = Arc::new(SharedDataImpl {
            single_data: RwLock::new(()),
            multiple_data: [(); T1_LEN].map(|_| RwLock::new(data.clone())),
            swap_index: AtomicUsize::new(0),
        });

        Self { inner }
    }
}

impl<T0, T1, const T1_LEN: usize> SharedData<T0, T1, T1_LEN> {
    pub fn new(single_data: T0, multiple_data: [T1; T1_LEN]) -> Self {
        let inner = Arc::new(SharedDataImpl {
            single_data: RwLock::new(single_data),
            multiple_data: multiple_data.map(|data| RwLock::new(data)),
            swap_index: AtomicUsize::new(0),
        });

        Self { inner }
    }

    pub async fn read_single(&self) -> RwLockReadGuard<'_, T0> {
        self.inner.single_data.read().await
    }

    pub async fn write_single(&mut self) -> RwLockWriteGuard<'_, T0> {
        self.inner.single_data.write().await
    }

    #[inline]
    pub fn read_multiple(&self, offset: usize) -> RwLockReadGuard<'_, T1> {
        debug_assert!(offset > 0);
        let index = (self.inner.swap_index.load(Ordering::Acquire) + offset) % T1_LEN;
        self.inner.multiple_data[index].try_read().unwrap()
    }

    #[inline]
    pub fn write_multiple(&mut self) -> RwLockWriteGuard<'_, T1> {
        let index = self.inner.swap_index.load(Ordering::Acquire);
        self.inner.multiple_data[index].try_write().unwrap()
    }

    pub fn swap_multiple(&mut self) {
        if T1_LEN > 0 {
            self.inner
                .swap_index
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |index| {
                    if index > 0 {
                        Some(index - 1)
                    } else {
                        Some(T1_LEN - 1)
                    }
                })
                .unwrap();
        }
    }
}
