// clippy false positive on condvar with mutexed counter
#![allow(clippy::mutex_atomic)]
#![feature(waker_getters)]

use std::{
    cell::{Cell, UnsafeCell},
    future::Future,
    marker::PhantomPinned,
    mem,
    num::NonZeroUsize,
    panic,
    pin::Pin,
    process, ptr,
    sync::{
        atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering},
        mpsc::{self, Sender},
        Arc, Condvar, Mutex,
    },
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    thread::{self, JoinHandle},
};

use core_affinity::{get_core_ids, CoreId};

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
    Task(Pin<&'static Task>),
    Join,
}

thread_local! {
    static TASK_SENDER: Cell<*const Sender<ChannelMessage>> = Cell::new(ptr::null())
}

#[derive(Default)]
struct BlockingTaskInfo {
    task: AtomicPtr<Task>,
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
        let default_hook = panic::take_hook();
        panic::set_hook(Box::new(move |err| {
            default_hook(err);
            process::exit(1);
        }));

        // SAFETY: we do not return until all the threads have run the registration callback
        let register_thread =
            unsafe { mem::transmute::<_, &'static (dyn Fn(usize) + Sync)>(register_thread) };

        struct ThreadInfo {
            init_count: Mutex<u8>,
            cvar: Condvar,
            core_ids: Vec<CoreId>,
        }

        let thread_init = Arc::new(ThreadInfo {
            init_count: Mutex::new(0),
            cvar: Condvar::new(),
            core_ids: get_core_ids().unwrap(),
        });

        if thread_count.get() > thread_init.core_ids.len() {
            log::warn!(
                "thread count ({}) > available thread core ids ({})",
                thread_init.core_ids.len(),
                thread_count.get(),
            );
        }

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
                if let Some(id) = thread_init.core_ids.get(thread_index) {
                    core_affinity::set_for_current(*id);
                }

                register_thread(thread_index);
                *thread_init.init_count.lock().unwrap() += 1;
                thread_init.cvar.notify_one();
                drop(thread_init);

                TASK_SENDER.with(|sender| sender.set(&task_sender));

                loop {
                    let task = task_receiver.lock().unwrap().recv().unwrap();
                    match task {
                        ChannelMessage::Task(task) => {
                            let ptr = &*task as *const _;

                            let waker =
                                RawWaker::new((&*task) as *const Task as *const (), &VTABLE);
                            let waker = unsafe { Waker::from_raw(waker) };
                            let mut context = Context::from_waker(&waker);

                            match task.poll_future(&mut context) {
                                TaskStatus::Ready => {
                                    if ptr == blocking_task_info.task.load(Ordering::Acquire) {
                                        *blocking_task_info.completed.lock().unwrap() = true;
                                        blocking_task_info.cvar.notify_one();
                                    }
                                }
                                TaskStatus::UnableToPoll => {
                                    // reinstert task to queue
                                    TASK_SENDER.with(|sender| unsafe {
                                        let sender = sender.get().as_ref().unwrap_unchecked();
                                        sender.send(ChannelMessage::Task(task)).unwrap();
                                    });
                                }
                                TaskStatus::Pending => {}
                            }
                        }
                        ChannelMessage::Join => break,
                    }
                }
            }));
        }

        let _init_guard = thread_init
            .cvar
            .wait_while(thread_init.init_count.lock().unwrap(), |count| {
                (*count as usize) < thread_count.get()
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
        let join_handle = AtomicUsize::default();

        // SAFETY: we block until the future completes.
        pin_unsafe!(join_handle, AtomicUsize);

        // SAFETY: we block until the future completes.
        let future: Pin<&'static mut (dyn Future<Output = ()> + Send)> =
            unsafe { mem::transmute(future) };

        let task = Task::new(future, join_handle);

        // SAFETY: we block until the future completes.
        pin_unsafe!(task, Task);

        self.blocking_task_info
            .task
            .store(&*task as *const Task as *mut _, Ordering::Release);

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

const VTABLE: RawWakerVTable = RawWakerVTable::new(task_clone, |_| {}, |_| {}, |_| {});

fn task_clone(task: *const ()) -> RawWaker {
    RawWaker::new(task, &VTABLE)
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

enum TaskStatus {
    Ready,
    Pending,
    UnableToPoll,
}

// We require at least an alignment of 2 so that the lower bit of the pointer may act as a flag.
// This allows "done" and the pointer to the parent task to be set in a single atomic operation.
#[repr(align(2))]
struct Task {
    future: UnsafeCell<Pin<&'static mut (dyn Future<Output = ()> + Send)>>,
    join_handle: Pin<&'static AtomicUsize>,
    executing: AtomicBool,
    // the waker system relies on stable Task addresses
    _pinned: PhantomPinned,
}

// SAFETY: access to non-sync values are protected by the `executing` atomic flag
unsafe impl Sync for Task {}

impl Task {
    fn new(
        future: Pin<&'static mut (dyn Future<Output = ()> + Send)>,
        join_handle: Pin<&'static AtomicUsize>,
    ) -> Self {
        Self {
            future: future.into(),
            join_handle,
            executing: AtomicBool::new(false),
            _pinned: PhantomPinned,
        }
    }

    fn poll_future(&self, context: &mut Context) -> TaskStatus {
        if self
            .executing
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            let future = unsafe { self.future.get().as_mut().unwrap_unchecked() };
            if future.as_mut().poll(context).is_ready() {
                let pending_task = self.join_handle.fetch_or(1, Ordering::SeqCst) as *const Task;
                if let Some(pending_task) = unsafe { pending_task.as_ref() } {
                    TASK_SENDER.with(|sender| unsafe {
                        let sender = sender.get().as_ref().unwrap_unchecked();
                        let task = mem::transmute(pending_task);
                        sender.send(ChannelMessage::Task(task)).unwrap();
                    });
                }

                TaskStatus::Ready
            } else {
                self.executing.store(false, Ordering::Release);
                TaskStatus::Pending
            }
        } else {
            TaskStatus::UnableToPoll
        }
    }
}

struct JoinHandleTask<'a> {
    join_handle: Pin<&'a AtomicUsize>,
}

impl<'a> Future for JoinHandleTask<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let task = cx.waker().as_raw().data() as usize;
        if self.join_handle.fetch_or(task, Ordering::SeqCst) & 1 == 1 {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
