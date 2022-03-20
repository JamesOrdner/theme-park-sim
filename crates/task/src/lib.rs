use std::{
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::Arc,
};

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};
use update_buffer::UpdateBufferRef;

#[derive(Clone)]
pub struct SharedData<T0 = (), T1 = (), const T1_LEN: usize = 0> {
    inner: Arc<SharedDataImpl<T0, T1, T1_LEN>>,
}

struct SharedDataImpl<T0 = (), T1 = (), const T1_LEN: usize = 0> {
    single_data: AtomicRefCell<T0>,
    multiple_data: [AtomicRefCell<T1>; T1_LEN],
}

impl<T0, T1, const T1_LEN: usize> Default for SharedData<T0, T1, T1_LEN>
where
    T0: Default,
    T1: Default,
{
    fn default() -> Self {
        let inner = Arc::new(SharedDataImpl {
            single_data: AtomicRefCell::new(Default::default()),
            multiple_data: [0; T1_LEN].map(|_| AtomicRefCell::new(Default::default())),
        });

        Self { inner }
    }
}

impl<T0, T1, const T1_LEN: usize> SharedData<T0, T1, T1_LEN>
where
    T0: Default,
    T1: Copy,
{
    pub fn new_multiple_copy(data: T1) -> Self {
        let inner = Arc::new(SharedDataImpl {
            single_data: AtomicRefCell::new(Default::default()),
            multiple_data: [0; T1_LEN].map(|_| AtomicRefCell::new(data)),
        });

        Self { inner }
    }
}

impl<T0, T1, const T1_LEN: usize> SharedData<T0, T1, T1_LEN>
where
    T0: Default,
    T1: Clone,
{
    pub fn new_multiple_clone(data: T1) -> Self {
        let inner = Arc::new(SharedDataImpl {
            single_data: AtomicRefCell::new(Default::default()),
            multiple_data: [0; T1_LEN].map(|_| AtomicRefCell::new(data.clone())),
        });

        Self { inner }
    }
}

impl<T0, T1, const T1_LEN: usize> SharedData<T0, T1, T1_LEN>
where
    T1: Default,
{
    pub fn new_single(data: T0) -> Self {
        let inner = Arc::new(SharedDataImpl {
            single_data: AtomicRefCell::new(data),
            multiple_data: [0; T1_LEN].map(|_| AtomicRefCell::new(Default::default())),
        });

        Self { inner }
    }
}

impl<T0, T1, const T1_LEN: usize> SharedData<T0, T1, T1_LEN> {
    pub fn new(single_data: T0, multiple_data: [T1; T1_LEN]) -> Self {
        let inner = Arc::new(SharedDataImpl {
            single_data: AtomicRefCell::new(single_data),
            multiple_data: multiple_data.map(|data| AtomicRefCell::new(data)),
        });

        Self { inner }
    }

    pub async fn read_single(&self) -> SharedDataRef<'_, T0> {
        SharedDataRef {
            inner: self.inner.single_data.borrow(),
        }
    }

    pub async fn write_single(&self) -> SharedDataRefMut<'_, T0> {
        SharedDataRefMut {
            inner: self.inner.single_data.borrow_mut(),
        }
    }

    pub async fn read_multiple(&self, offset: usize) -> SharedDataRef<'_, T1> {
        SharedDataRef {
            inner: self.inner.multiple_data[offset].borrow(),
        }
    }

    pub async fn write_multiple(&self, offset: usize) -> SharedDataRefMut<'_, T1> {
        SharedDataRefMut {
            inner: self.inner.multiple_data[offset].borrow_mut(),
        }
    }
}

pub struct SharedDataRef<'a, T> {
    inner: AtomicRef<'a, T>,
}

impl<'a, T> Deref for SharedDataRef<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

pub struct SharedDataRefMut<'a, T> {
    inner: AtomicRefMut<'a, T>,
}

impl<'a, T> Deref for SharedDataRefMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

impl<'a, T> DerefMut for SharedDataRefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.inner
    }
}

pub trait FixedUpdateTask {
    /// eventually can be async fn
    fn task<'a>(
        self: Pin<&'a mut Self>,
        update_buffer: &UpdateBufferRef,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}
