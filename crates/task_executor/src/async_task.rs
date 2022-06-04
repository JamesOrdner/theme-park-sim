use std::{
    cell::UnsafeCell,
    future::Future,
    mem::{self, MaybeUninit},
    pin::Pin,
    sync::{atomic::Ordering, Arc},
};

use crate::{AtomicUsize, ChannelMessage, Task, TASK_SENDER};

pub struct AsyncTaskHandle<T> {
    _future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    join_handle: Pin<Box<AtomicUsize>>,
    _task: Pin<Box<Task>>,
    result: Arc<TaskResultWrapper<T>>,
}

#[derive(Debug)]
struct TaskResultWrapper<T> {
    inner: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T> Sync for TaskResultWrapper<T> where T: Send {}

impl<T> Default for TaskResultWrapper<T> {
    fn default() -> Self {
        Self {
            inner: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

impl<T> AsyncTaskHandle<T> {
    pub fn result(self) -> Result<T, Self> {
        if self.join_handle.load(Ordering::Acquire) & 1 == 1 {
            match Arc::try_unwrap(self.result) {
                Ok(result) => unsafe { Ok(result.inner.into_inner().assume_init()) },
                Err(_) => panic!(),
            }
        } else {
            Err(self)
        }
    }
}

pub fn execute_async<F, T>(future: F) -> AsyncTaskHandle<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let result = Arc::new(TaskResultWrapper::default());
    let mut future = {
        let result = result.clone();
        Box::pin(async move {
            let awaited = future.await;
            unsafe {
                result
                    .inner
                    .get()
                    .as_mut()
                    .unwrap_unchecked()
                    .write(awaited);
            }
        })
    };

    let join_handle = Box::pin(AtomicUsize::default());

    let join_handle_ref: Pin<&AtomicUsize> = join_handle.as_ref();
    let join_handle_ref: Pin<&'static AtomicUsize> = unsafe { mem::transmute(join_handle_ref) };

    let task = unsafe {
        let future: Pin<&mut dyn Future<Output = ()>> = future.as_mut();
        let future: Pin<&'static mut (dyn Future<Output = ()> + Send)> = mem::transmute(future);
        Box::pin(Task::new(future, join_handle_ref))
    };

    let task_ref: Pin<&Task> = task.as_ref();
    let task_ref: Pin<&'static Task> = unsafe { mem::transmute(task_ref) };

    // begin execution of task
    TASK_SENDER.with(|sender| {
        // must be called from an executor thread
        debug_assert!(!sender.get().is_null());

        let sender = unsafe { sender.get().as_ref().unwrap_unchecked() };
        sender.send(ChannelMessage::Task(task_ref)).unwrap();
    });

    AsyncTaskHandle {
        _future: future,
        join_handle,
        _task: task,
        result,
    }
}
