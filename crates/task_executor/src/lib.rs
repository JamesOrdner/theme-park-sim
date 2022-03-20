use std::{
    future::Future,
    mem,
    num::NonZeroUsize,
    pin::Pin,
    ptr,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    thread,
};

use task::FixedUpdateTask;
use update_buffer::UpdateBufferRef;

pub struct TaskExecutor;

impl TaskExecutor {
    pub fn available_parallelism() -> NonZeroUsize {
        thread::available_parallelism().expect("unable to determine available parallelism")
    }

    pub fn execute_blocking<T>(&mut self, task: &mut T)
    where
        T: Future<Output = ()> + Send,
    {
        // SAFETY: future guaranteed not to move in the scope of this function
        let mut task = unsafe { Pin::new_unchecked(task) };

        let waker = RawWaker::new(ptr::null_mut() as *mut (), &VTABLE);
        let waker = unsafe { Waker::from_raw(waker) };

        match task.as_mut().poll(&mut Context::from_waker(&waker)) {
            Poll::Ready(_) => {}
            Poll::Pending => panic!(),
        }
    }

    pub fn fixed_update_executor(&self, update_buffer: UpdateBufferRef) -> FixedUpdateExecutor {
        FixedUpdateExecutor {
            _executor: self,
            update_buffer,
        }
    }
}

const VTABLE: RawWakerVTable = {
    RawWakerVTable::new(
        |s| RawWaker::new(s as *const (), &VTABLE),
        |_| {},
        |_| {},
        |_| {},
    )
};

pub struct FixedUpdateExecutor<'a> {
    _executor: &'a TaskExecutor,
    update_buffer: UpdateBufferRef,
}

impl<'a> FixedUpdateExecutor<'a> {
    pub fn execute_async<T>(&self, mut task: Pin<Box<T>>) -> FixedUpdateTaskHandle<T>
    where
        T: FixedUpdateTask,
    {
        // SAFETY: future will only ever be polled as long as the task data is
        // valid. We also ensure that the future is dropped before the data.
        let future = unsafe { mem::transmute(task.as_mut().task(&self.update_buffer)) };

        // TODO: start task processing

        FixedUpdateTaskHandle {
            future,
            data: Some(task),
        }
    }
}

pub struct FixedUpdateTaskHandle<T> {
    future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    data: Option<Pin<Box<T>>>,
}

impl<T> Future for FixedUpdateTaskHandle<T> {
    type Output = Pin<Box<T>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.future.as_mut().poll(cx) {
            Poll::Ready(_) => Poll::Ready(self.data.take().unwrap()),
            Poll::Pending => Poll::Pending,
        }
    }
}
