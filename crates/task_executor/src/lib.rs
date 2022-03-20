use std::{
    future::Future,
    mem,
    num::NonZeroUsize,
    pin::Pin,
    task::{Context, Poll},
    thread,
};

use task::FixedUpdateTask;

pub struct TaskExecutor;

impl TaskExecutor {
    pub fn available_parallelism() -> NonZeroUsize {
        thread::available_parallelism().expect("unable to determine available parallelism")
    }

    pub fn execute_blocking<T>(&mut self, _task: &mut T)
    where
        T: Future<Output = ()> + Send,
    {
        // SAFETY: future guaranteed not to move in the scope of this function
        let _task = unsafe { Pin::new_unchecked(_task) };
    }
}

pub struct FixedTaskExecutor;

impl FixedTaskExecutor {
    pub fn execute_async<T>(&self, mut task: Pin<Box<T>>) -> FixedUpdateTaskHandle<T>
    where
        T: FixedUpdateTask,
    {
        // SAFETY: future will only ever be polled as long as the task data is
        // valid. We also ensure that the future is dropped before the data.
        let future = unsafe { mem::transmute(task.as_mut().task()) };

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
