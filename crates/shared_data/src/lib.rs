use std::sync::Arc;

use async_lock::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Clone)]
pub struct SharedData<T0 = (), T1 = (), const T1_LEN: usize = 0> {
    inner: Arc<SharedDataImpl<T0, T1, T1_LEN>>,
}

struct SharedDataImpl<T0 = (), T1 = (), const T1_LEN: usize = 0> {
    single_data: RwLock<T0>,
    multiple_data: [RwLock<T1>; T1_LEN],
}

impl<T0> SharedData<T0, (), 0> {
    pub fn new_single(data: T0) -> Self {
        let inner = Arc::new(SharedDataImpl {
            single_data: RwLock::new(data),
            multiple_data: [],
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
        });

        Self { inner }
    }
}

impl<T0, T1, const T1_LEN: usize> SharedData<T0, T1, T1_LEN> {
    pub fn new(single_data: T0, multiple_data: [T1; T1_LEN]) -> Self {
        let inner = Arc::new(SharedDataImpl {
            single_data: RwLock::new(single_data),
            multiple_data: multiple_data.map(|data| RwLock::new(data)),
        });

        Self { inner }
    }

    pub async fn read_single(&self) -> RwLockReadGuard<'_, T0> {
        self.inner.single_data.read().await
    }

    pub async fn write_single(&self) -> RwLockWriteGuard<'_, T0> {
        self.inner.single_data.write().await
    }

    pub async fn read_multiple(&self, offset: usize) -> RwLockReadGuard<'_, T1> {
        self.inner.multiple_data[offset].read().await
    }

    pub async fn write_multiple(&self, offset: usize) -> RwLockWriteGuard<'_, T1> {
        self.inner.multiple_data[offset].write().await
    }
}
