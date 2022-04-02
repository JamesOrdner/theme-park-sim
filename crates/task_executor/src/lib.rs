// clippy false positive on condvar with mutexed counter
#![allow(clippy::mutex_atomic)]

use std::{
    cell::Cell,
    future::Future,
    iter::zip,
    mem::{self, MaybeUninit},
    num::NonZeroUsize,
    pin::Pin,
    ptr,
    sync::{
        atomic::{AtomicPtr, Ordering},
        mpsc::{self, Sender},
        Arc, Condvar, Mutex,
    },
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    thread::{self, JoinHandle},
};

use spin::Mutex as SpinMutex;

macro_rules! pin {
    ($a:ident) => {
        // Move the value to ensure that it is owned
        let $a = $a;
        let $a = unsafe { Pin::new_unchecked(&$a) };
    };
}

enum ChannelMessage {
    Task(&'static mut Task<'static>),
    Join,
}

thread_local! {
    static TASK_SENDER: Cell<*const Sender<ChannelMessage>> = Cell::new(ptr::null())
}

struct BlockingTaskInfo {
    task: AtomicPtr<Task<'static>>,
    cvar: Condvar,
    completed: Mutex<bool>,
}

pub struct TaskExecutor {
    task_sender: Sender<ChannelMessage>,
    blocking_task_info: Arc<BlockingTaskInfo>,
    thread_join_handles: Vec<JoinHandle<()>>,
}

impl TaskExecutor {
    pub fn new(thread_count: NonZeroUsize, register_thread: &(dyn Fn(usize) + Sync)) -> Self {
        // SAFETY: we do not return until all the threads have run the registration callback
        let register_thread =
            unsafe { mem::transmute::<_, &'static (dyn Fn(usize) + Sync)>(register_thread) };

        let thread_init = Arc::new((Mutex::new(0), Condvar::new()));

        let (task_sender, task_receiver) = mpsc::channel();
        let task_receiver = Arc::new(Mutex::new(task_receiver));

        let blocking_task_info = Arc::new(BlockingTaskInfo {
            task: AtomicPtr::new(ptr::null_mut()),
            cvar: Condvar::new(),
            completed: Mutex::new(false),
        });

        let mut thread_join_handles = Vec::with_capacity(thread_count.get());

        for thread_index in 0..thread_count.get() {
            let thread_init = thread_init.clone();

            let task_sender = task_sender.clone();
            let task_receiver = task_receiver.clone();
            let blocking_task_info = blocking_task_info.clone();

            thread_join_handles.push(thread::spawn(move || {
                register_thread(thread_index);
                *thread_init.0.lock().unwrap() += 1;
                thread_init.1.notify_one();
                drop(thread_init);

                TASK_SENDER.with(|sender| sender.set(&task_sender));

                loop {
                    let task = task_receiver.lock().unwrap().recv().unwrap();
                    match task {
                        ChannelMessage::Task(task) => {
                            if task.poll_future()
                                && task as *mut _ == blocking_task_info.task.load(Ordering::Acquire)
                            {
                                *blocking_task_info.completed.lock().unwrap() = true;
                                blocking_task_info.cvar.notify_one();
                            }
                        }
                        ChannelMessage::Join => break,
                    }
                }
            }));
        }

        let _init_guard = thread_init
            .1
            .wait_while(thread_init.0.lock().unwrap(), |count| {
                *count < thread_count.get()
            })
            .unwrap();

        Self {
            task_sender,
            blocking_task_info,
            thread_join_handles,
        }
    }

    pub fn available_parallelism() -> NonZeroUsize {
        thread::available_parallelism().expect("unable to determine available parallelism")
    }

    pub fn execute_blocking(&mut self, future: Pin<&mut (dyn Future<Output = ()> + Send)>) {
        let join_handle = SpinMutex::default();
        pin!(join_handle);

        let mut task = Task {
            future,
            join_handle,
        };

        // SAFETY: we block until the future completes, and shadow the
        // task variable to ensure that we don't alias mutable borrows.
        // TODO: will multiple wakers ever reference this task simultaneously?
        let task: &'static mut _ = unsafe { mem::transmute(&mut task) };

        self.blocking_task_info.task.store(task, Ordering::Release);

        self.task_sender.send(ChannelMessage::Task(task)).unwrap();

        let mut task_guard = self
            .blocking_task_info
            .cvar
            .wait_while(
                self.blocking_task_info.completed.lock().unwrap(),
                |completed| !*completed,
            )
            .unwrap();

        *task_guard = false;
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

impl Drop for TaskExecutor {
    fn drop(&mut self) {
        for _ in 0..self.thread_join_handles.len() {
            self.task_sender.send(ChannelMessage::Join).unwrap();
        }

        for thread in self.thread_join_handles.drain(..) {
            thread.join().unwrap();
        }
    }
}

fn task_clone(s: *const Task) -> RawWaker {
    RawWaker::new(s as *const (), &VTABLE)
}

fn task_wake(task: *const Task) {
    // SAFETY: this is still an exclusive reference. It has only
    // been cast to *const to please the waker API
    let task = unsafe { (task as *mut Task).as_mut().unwrap() };

    TASK_SENDER.with(|sender| {
        let sender = unsafe { sender.get().as_ref().unwrap_unchecked() };
        sender.send(ChannelMessage::Task(task)).unwrap();
    });
}

const VTABLE: RawWakerVTable = {
    RawWakerVTable::new(
        |s| task_clone(s as *const Task),
        |s| task_wake(s as *const Task),
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

macro_rules! pin_array {
    ($arr: ident, $len: expr) => {
        let $arr = {
            unsafe {
                let mut x: [MaybeUninit<_>; $len] = MaybeUninit::uninit().assume_init();
                for (i, a) in $arr.iter().enumerate() {
                    x[i].write(Pin::new_unchecked(a));
                }
                x.map(|a| a.assume_init())
            }
        };
    };
}

pub async fn parallel<const N: usize>(futures: [Pin<&mut (dyn Future<Output = ()> + Send)>; N]) {
    let join_handles = [0; N].map(|_| SpinMutex::new(TaskJoinHandle::default()));
    pin_array!(join_handles, N);

    let mut tasks = unsafe {
        let mut tasks: [MaybeUninit<_>; N] = MaybeUninit::uninit().assume_init();
        for (task, (future, join_handle)) in zip(&mut tasks, zip(futures, join_handles)) {
            task.write(Task {
                future,
                join_handle,
            });
        }
        tasks.map(|a| a.assume_init())
    };

    TASK_SENDER.with(|sender| {
        let sender = unsafe { sender.get().as_ref().unwrap_unchecked() };
        for task in tasks.iter_mut() {
            // SAFETY: we join the tasks' futures before returning
            let task = unsafe { mem::transmute(task) };
            sender.send(ChannelMessage::Task(task)).unwrap();
        }
    });

    for join_handle in join_handles {
        JoinHandleTask { join_handle }.await;
    }
}

struct Task<'a> {
    future: Pin<&'a mut (dyn Future<Output = ()> + Send)>,
    join_handle: Pin<&'a SpinMutex<TaskJoinHandle>>,
}

impl<'a> Task<'a> {
    fn poll_future(&mut self) -> bool {
        let waker = RawWaker::new(self as *mut Task as *const (), &VTABLE);
        let waker = unsafe { Waker::from_raw(waker) };

        match self.future.as_mut().poll(&mut Context::from_waker(&waker)) {
            Poll::Ready(_) => {
                let mut join_handle = self.join_handle.lock();

                join_handle.done = true;

                if let Some(waker) = join_handle.waker.take() {
                    waker.wake();
                }

                true
            }
            Poll::Pending => false,
        }
    }
}

#[derive(Default)]
struct TaskJoinHandle {
    done: bool,
    waker: Option<Waker>,
}

struct JoinHandleTask<'a> {
    join_handle: Pin<&'a SpinMutex<TaskJoinHandle>>,
}

impl<'a> Future for JoinHandleTask<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut join_handle = self.join_handle.lock();

        if join_handle.done {
            Poll::Ready(())
        } else {
            join_handle.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
