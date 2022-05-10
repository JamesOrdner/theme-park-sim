// clippy false positive on condvar with mutexed counter
#![allow(clippy::mutex_atomic)]

use std::{
    cell::Cell,
    future::Future,
    marker::PhantomPinned,
    mem,
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

pub mod async_task;
pub mod task;

/// Stack-pins a value and extends the reference lifetime to 'static.
macro_rules! pin_unsafe {
    ($a:ident, $t:ty) => {
        // Move the value to ensure that it is owned
        let $a = $a;
        let $a: Pin<&$t> = unsafe { Pin::new_unchecked(&$a) };
        let $a: Pin<&'static $t> = unsafe { mem::transmute($a) };
    };
}

/// Stack-pins an array and extends the reference lifetimes to 'static.
macro_rules! pin_array_unsafe {
    ($arr: ident, $len: ident, $t:ty) => {
        // Move the array to ensure that it is owned
        let $arr = $arr;

        let $arr = {
            unsafe {
                let mut x: [MaybeUninit<_>; $len] = MaybeUninit::uninit().assume_init();
                for (x, a) in zip(&mut x, &$arr) {
                    let a: Pin<&$t> = Pin::new_unchecked(a);
                    let a: Pin<&'static $t> = mem::transmute(a);
                    x.write(a);
                }
                x.map(|a| a.assume_init())
            }
        };
    };
}

pub(crate) use pin_array_unsafe;

enum ChannelMessage {
    Task(Pin<&'static SpinMutex<Task>>),
    Join,
}

thread_local! {
    static TASK_SENDER: Cell<*const Sender<ChannelMessage>> = Cell::new(ptr::null())
}

#[derive(Default)]
struct BlockingTaskInfo {
    task: AtomicPtr<SpinMutex<Task>>,
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

        let blocking_task_info = Arc::new(BlockingTaskInfo::default());

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
                            let ptr = &*task as *const _;

                            let waker = RawWaker::new(
                                (&*task) as *const SpinMutex<Task> as *const (),
                                &VTABLE,
                            );
                            let waker = unsafe { Waker::from_raw(waker) };
                            let mut context = Context::from_waker(&waker);

                            let ready = task.lock().poll_future(&mut context);
                            if ready && ptr == blocking_task_info.task.load(Ordering::Acquire) {
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

        // SAFETY: we block until the future completes.
        pin_unsafe!(join_handle, SpinMutex<TaskJoinHandle>);

        // SAFETY: we block until the future completes.
        let future: Pin<&'static mut (dyn Future<Output = ()> + Send)> =
            unsafe { mem::transmute(future) };

        let task = SpinMutex::new(Task::new(future, join_handle));

        // SAFETY: we block until the future completes.
        pin_unsafe!(task, SpinMutex<Task>);

        self.blocking_task_info.task.store(
            &*task as *const SpinMutex<Task> as *mut _,
            Ordering::Release,
        );

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

    pub fn execute_fixed<F, T>(&mut self, task: F) -> FixedTaskHandle<T>
    where
        F: Future<Output = Box<T>> + Send + 'static,
    {
        // TODO: start task processing

        FixedTaskHandle {
            future: Box::pin(task),
        }
    }
}

impl Drop for TaskExecutor {
    fn drop(&mut self) {
        for _ in &self.thread_join_handles {
            self.task_sender.send(ChannelMessage::Join).unwrap();
        }

        for thread in self.thread_join_handles.drain(..) {
            thread.join().unwrap();
        }
    }
}

const VTABLE: RawWakerVTable = RawWakerVTable::new(task_clone, task_wake, |_| {}, |_| {});

fn task_clone(task: *const ()) -> RawWaker {
    RawWaker::new(task, &VTABLE)
}

fn task_wake(task: *const ()) {
    let task = unsafe {
        let task = (task as *const SpinMutex<Task>).as_ref().unwrap_unchecked();
        Pin::new_unchecked(task)
    };

    TASK_SENDER.with(|sender| {
        let sender = unsafe { sender.get().as_ref().unwrap_unchecked() };
        sender.send(ChannelMessage::Task(task)).unwrap();
    });
}

pub struct FixedTaskHandle<T> {
    future: Pin<Box<dyn Future<Output = Box<T>> + Send + 'static>>,
}

impl<T> Future for FixedTaskHandle<T> {
    type Output = Box<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.future.as_mut().poll(cx)
    }
}

struct Task {
    future: Pin<&'static mut (dyn Future<Output = ()> + Send)>,
    join_handle: Pin<&'static SpinMutex<TaskJoinHandle>>,
    // the waker system relies on stable Task addresses
    _pinned: PhantomPinned,
}

impl Task {
    fn new(
        future: Pin<&'static mut (dyn Future<Output = ()> + Send)>,
        join_handle: Pin<&'static SpinMutex<TaskJoinHandle>>,
    ) -> Self {
        Self {
            future,
            join_handle,
            _pinned: PhantomPinned,
        }
    }

    fn poll_future(&mut self, context: &mut Context) -> bool {
        if self.future.as_mut().poll(context).is_ready() {
            let mut join_handle = self.join_handle.lock();

            join_handle.done = true;

            if let Some(waker) = join_handle.waker.take() {
                waker.wake();
            }

            true
        } else {
            false
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
