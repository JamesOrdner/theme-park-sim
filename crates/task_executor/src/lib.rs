// clippy false positive on condvar with mutexed counter
#![allow(clippy::mutex_atomic)]

use std::{
    future::Future,
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

use futures::pin_mut;

enum ChannelMessage {
    Task(*mut Task),
    Join,
}

// SAFETY: Task is Send
unsafe impl Send for ChannelMessage {}

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
        let (task_sender, task_receiver) = mpsc::channel();
        let task_receiver = Arc::new(Mutex::new(task_receiver));

        // SAFETY: cast to 'static is safe because we do not return until
        // all the threads have run the registration callback
        let register_thread =
            unsafe { mem::transmute::<_, &'static (dyn Fn(usize) + Sync)>(register_thread) };

        let thread_init = Arc::new((Mutex::new(0), Condvar::new()));

        let blocking_task_info = Arc::new(BlockingTaskInfo {
            task: AtomicPtr::new(ptr::null_mut()),
            cvar: Condvar::new(),
            completed: Mutex::new(false),
        });

        let mut thread_join_handles = Vec::with_capacity(thread_count.get());

        for thread_index in 0..thread_count.get() {
            let task_receiver = task_receiver.clone();
            let thread_init = thread_init.clone();
            let blocking_task_info = blocking_task_info.clone();

            thread_join_handles.push(thread::spawn(move || {
                register_thread(thread_index);
                *thread_init.0.lock().unwrap() += 1;
                thread_init.1.notify_one();
                drop(thread_init);

                loop {
                    let task = task_receiver.lock().unwrap().recv().unwrap();
                    match task {
                        ChannelMessage::Task(task_ptr) => {
                            // SAFETY: we only ever create a Task reference here
                            let task = unsafe { task_ptr.as_mut().unwrap() };
                            if task.poll_future()
                                && task_ptr == blocking_task_info.task.load(Ordering::Acquire)
                            {
                                let mut task_guard = blocking_task_info.completed.lock().unwrap();
                                *task_guard = true;
                                blocking_task_info.cvar.notify_one();
                            }
                        }
                        ChannelMessage::Join => break,
                    }
                }
            }));
        }

        let _guard = thread_init
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

    pub fn execute_blocking<T>(&mut self, future: T)
    where
        T: Future<Output = ()> + Send,
    {
        pin_mut!(future);

        let task = Task::new(future);
        pin_mut!(task); // do we need to pin this?

        self.blocking_task_info
            .task
            .store(&mut *task, Ordering::Release);

        self.task_sender
            .send(ChannelMessage::Task(&mut *task))
            .unwrap();

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

pub async fn parallel<const N: usize>(futures: [Pin<&mut (dyn Future<Output = ()> + Send)>; N]) {
    // TODO: parallel task execution

    for future in futures {
        future.await;
    }
}

struct Task {
    future: Pin<&'static mut (dyn Future<Output = ()> + Send)>,
    _join_handle: *const (),
}

impl Task {
    fn new(future: Pin<&mut (dyn Future<Output = ()> + Send)>) -> Self {
        // SAFETY: all task-running functions join futures before returning
        let future: Pin<&'static mut _> = unsafe { mem::transmute(future) };

        Self {
            future,
            _join_handle: ptr::null(),
        }
    }
}

impl Task {
    fn poll_future(&mut self) -> bool {
        let waker = RawWaker::new(ptr::null_mut() as *mut (), &VTABLE);
        let waker = unsafe { Waker::from_raw(waker) };

        match self.future.as_mut().poll(&mut Context::from_waker(&waker)) {
            Poll::Ready(_) => true,
            Poll::Pending => panic!(),
        }
    }
}
