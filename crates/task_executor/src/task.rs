use std::{
    future::Future,
    iter::zip,
    mem::{self, MaybeUninit},
    pin::Pin,
};

use spin::mutex::Mutex as SpinMutex;

use crate::{pin_array_unsafe, ChannelMessage, JoinHandleTask, Task, TaskJoinHandle, TASK_SENDER};

pub async fn parallel<const N: usize>(futures: [Pin<&mut (dyn Future<Output = ()> + Send)>; N]) {
    let join_handles = [0; N].map(|_| SpinMutex::default());

    // SAFETY: we block until the future completes.
    pin_array_unsafe!(join_handles, N, SpinMutex<TaskJoinHandle>);

    let tasks = unsafe {
        let mut tasks: [MaybeUninit<_>; N] = MaybeUninit::uninit().assume_init();
        for (task, (future, join_handle)) in zip(&mut tasks, zip(futures, join_handles)) {
            // SAFETY: we block until the future completes.
            let future: Pin<&'static mut (dyn Future<Output = ()> + Send)> = mem::transmute(future);
            task.write(SpinMutex::new(Task::new(future, join_handle)));
        }
        tasks.map(|a| a.assume_init())
    };

    // SAFETY: we block until the future completes.
    pin_array_unsafe!(tasks, N, SpinMutex<Task>);

    TASK_SENDER.with(|sender| {
        let sender = unsafe { sender.get().as_ref().unwrap_unchecked() };
        for task in tasks {
            sender.send(ChannelMessage::Task(task)).unwrap();
        }
    });

    for join_handle in join_handles {
        JoinHandleTask { join_handle }.await;
    }

    // TODO: ensure tasks persist across this await point
}
