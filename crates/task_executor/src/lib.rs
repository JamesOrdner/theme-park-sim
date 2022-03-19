use std::{future::Future, num::NonZeroUsize, pin::Pin, thread};

pub struct TaskExecutor;

impl TaskExecutor {
    pub fn max_parallelism() -> NonZeroUsize {
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
