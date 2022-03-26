use std::{
    future::Future,
    num::NonZeroUsize,
    pin::Pin,
    ptr,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    thread,
};

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

    pub fn execute_async<T, U>(&mut self, task: T) -> FixedUpdateTaskHandle<U>
    where
        T: Future<Output = Box<U>> + Send + 'static,
    {
        // TODO: start task processing

        FixedUpdateTaskHandle {
            future: Box::pin(task),
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

pub struct FixedUpdateTaskHandle<T> {
    future: Pin<Box<dyn Future<Output = Box<T>> + Send + 'static>>,
}

impl<T> Future for FixedUpdateTaskHandle<T> {
    type Output = Box<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.future.as_mut().poll(cx) {
            Poll::Ready(result) => Poll::Ready(result),
            Poll::Pending => Poll::Pending,
        }
    }
}
