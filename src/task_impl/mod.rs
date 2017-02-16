use std::prelude::v1::*;

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{Ordering, AtomicBool, AtomicUsize, ATOMIC_USIZE_INIT};
use std::thread;

use {Poll, Future, Async};
use future::BoxFuture;
use stream::Stream;

mod unpark_mutex;
use self::unpark_mutex::UnparkMutex;

fn fresh_task_id() -> usize {
    // TODO: this assert is a real bummer, need to figure out how to reuse
    //       old IDs that are no longer in use.
    static NEXT_ID: AtomicUsize = ATOMIC_USIZE_INIT;
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    assert!(id < usize::max_value() / 2,
            "too many previous tasks have been allocated");
    id
}

/// A handle to a "task", which represents a single lightweight "thread" of
/// execution driving a future to completion.
///
/// In general, futures are composed into large units of work, which are then
/// spawned as tasks onto an *executor*. The executor is responsible for polling
/// the future as notifications arrive, until the future terminates.
///
/// This is obtained by the `task::park` function.
#[derive(Clone)]
pub struct Task {
    id: usize,
    unpark: Arc<Unpark>,
    events: Events,
}

fn _assert_kinds() {
    fn _assert_send<T: Send>() {}
    _assert_send::<Task>();
}

/// Creates a new Task that ignores unpark events.
pub fn empty() -> Task {
    Task {
        id: fresh_task_id(),
        unpark: Arc::new(IgnoreUnpark),
        events: Events::new(),
    }
}

impl Task {
    /// Indicate that the task should attempt to poll its future in a timely
    /// fashion.
    ///
    /// It's typically guaranteed that, for each call to `unpark`, `poll` will
    /// be called at least once subsequently (unless the task has terminated).
    /// If the task is currently polling its future when `unpark` is called, it
    /// must poll the future *again* afterwards, ensuring that all relevant
    /// events are eventually observed by the future.
    pub fn unpark(&self) {
        self.events.trigger();
        self.unpark.unpark();
    }

    /// Unpark events are used to pass information about what event caused a task to
    /// be unparked. In some cases, tasks are waiting on a large number of possible
    /// events, and need precise information about the wakeup to avoid extraneous
    /// polling.
    ///
    /// Every `Task` handle comes with a set of unpark events which will fire when
    /// `unpark` is called. When fired, these events insert an identifer into a
    /// concurrent set, which the task can read from to determine what events
    /// occurred.
    pub fn with_unpark_event(&self, event: UnparkEvent) -> Task {
        Task {
            id: self.id,
            unpark: self.unpark.clone(),
            events: self.events.with_event(event),
        }
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Task")
         .field("id", &self.id)
         .finish()
    }
}

#[derive(Clone)]
/// A set insertion to trigger upon `unpark`.
///
/// Unpark events are used to communicate information about *why* an unpark
/// occured, in particular populating sets with event identifiers so that the
/// unparked task can avoid extraneous polling. See `with_unpark_event` for
/// more.
pub struct UnparkEvent {
    set: Arc<EventSet>,
    item: usize,
}

impl UnparkEvent {
    /// Construct an unpark event that will insert `id` into `set` when
    /// triggered.
    pub fn new(set: Arc<EventSet>, id: usize) -> UnparkEvent {
        UnparkEvent {
            set: set,
            item: id,
        }
    }
}

/// A concurrent set which allows for the insertion of `usize` values.
///
/// `EventSet`s are used to communicate precise information about the event(s)
/// that trigged a task notification. See `task::with_unpark_event` for details.
pub trait EventSet: Send + Sync + 'static {
    /// Insert the given ID into the set
    fn insert(&self, id: usize);
}

// A collection of UnparkEvents to trigger on `unpark`
#[derive(Clone)]
enum Events {
    Zero,
    One(UnparkEvent),
    Lots(Vec<UnparkEvent>),
}

impl Events {
    fn new() -> Events {
        Events::Zero
    }

    fn trigger(&self) {
        match *self {
            Events::Zero => {}
            Events::One(ref event) => event.set.insert(event.item),
            Events::Lots(ref list) => {
                for event in list {
                    event.set.insert(event.item);
                }
            }
        }
    }

    fn with_event(&self, event: UnparkEvent) -> Events {
        let mut list = match *self {
            Events::Zero => return Events::One(event),
            Events::One(ref event) => vec![event.clone()],
            Events::Lots(ref list) => list.clone(),
        };
        list.push(event);
        Events::Lots(list)
    }
}

/// Representation of a spawned future/stream.
///
/// This object is returned by the `spawn` function in this module. This
/// represents a "fused task and future", storing all necessary pieces of a task
/// and owning the top-level future that's being driven as well.
///
/// A `Spawn` can be poll'd for completion or execution of the current thread
/// can be blocked indefinitely until a notification arrives. This can be used
/// with either futures or streams, with different methods being available on
/// `Spawn` depending which is used.
pub struct Spawn<T> {
    obj: T,
    id: usize,
}

/// Spawns a new future, returning the fused future and task.
///
/// This function is the termination endpoint for running futures. This method
/// will conceptually allocate a new task to run the given object, which is
/// normally either a `Future` or `Stream`.
///
/// This function is similar to the `thread::spawn` function but does not
/// attempt to run code in the background. The future will not make progress
/// until the methods on `Spawn` are called in turn.
pub fn spawn<T>(obj: T) -> Spawn<T> {
    Spawn {
        obj: obj,
        id: fresh_task_id(),
    }
}

impl<T> Spawn<T> {
    /// Get a shared reference to the object the Spawn is wrapping.
    pub fn get_ref(&self) -> &T {
        &self.obj
    }

    /// Get a mutable reference to the object the Spawn is wrapping.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.obj
    }

    /// Consume the Spawn, returning its inner object
    pub fn into_inner(self) -> T {
        self.obj
    }
}

impl<F: Future> Spawn<F> {
    /// Polls the internal future, scheduling notifications to be sent to the
    /// `unpark` argument.
    ///
    /// This method will poll the internal future, testing if it's completed
    /// yet. The `unpark` argument is used as a sink for notifications sent to
    /// this future. That is, while the future is being polled, any call to
    /// `task::park()` will return a handle that contains the `unpark`
    /// specified.
    ///
    /// If this function returns `NotReady`, then the `unpark` should have been
    /// scheduled to receive a notification when poll can be called again.
    /// Otherwise if `Ready` or `Err` is returned, the `Spawn` task can be
    /// safely destroyed.
    pub fn poll_future(&mut self, unpark: Arc<Unpark>) -> Poll<F::Item, F::Error> {
        self.enter(unpark, |f, task| f.poll(task))
    }

    /// Waits for the internal future to complete, blocking this thread's
    /// execution until it does.
    ///
    /// This function will call `poll_future` in a loop, waiting for the future
    /// to complete. When a future cannot make progress it will use
    /// `thread::park` to block the current thread.
    pub fn wait_future(&mut self) -> Result<F::Item, F::Error> {
        let unpark = Arc::new(ThreadUnpark::new(thread::current()));
        loop {
            match try!(self.poll_future(unpark.clone())) {
                Async::NotReady => unpark.park(),
                Async::Ready(e) => return Ok(e),
            }
        }
    }

    /// A specialized function to request running a future to completion on the
    /// specified executor.
    ///
    /// This function only works for futures whose item and error types are `()`
    /// and also implement the `Send` and `'static` bounds. This will submit
    /// units of work (instances of `Run`) to the `exec` argument provided
    /// necessary to drive the future to completion.
    ///
    /// When the future would block, it's arranged that when the future is again
    /// ready it will submit another unit of work to the `exec` provided. This
    /// will happen in a loop until the future has completed.
    ///
    /// This method is not appropriate for all futures, and other kinds of
    /// executors typically provide a similar function with perhaps relaxed
    /// bounds as well.
    pub fn execute(self, exec: Arc<Executor>)
        where F: Future<Item=(), Error=()> + Send + 'static,
    {
        exec.clone().execute(Run {
            // Ideally this method would be defined directly on
            // `Spawn<BoxFuture<(), ()>>` so we wouldn't have to box here and
            // it'd be more explicit, but unfortunately that currently has a
            // link error on nightly: rust-lang/rust#36155
            spawn: Spawn {
                id: self.id,
                obj: self.obj.boxed(),
            },
            inner: Arc::new(Inner {
                exec: exec,
                mutex: UnparkMutex::new()
            }),
        })
    }
}

impl<S: Stream> Spawn<S> {
    /// Like `poll_future`, except polls the underlying stream.
    pub fn poll_stream(&mut self, unpark: Arc<Unpark>)
                       -> Poll<Option<S::Item>, S::Error> {
        self.enter(unpark, |stream, task| stream.poll(task))
    }

    /// Like `wait_future`, except only waits for the next element to arrive on
    /// the underlying stream.
    pub fn wait_stream(&mut self) -> Option<Result<S::Item, S::Error>> {
        let unpark = Arc::new(ThreadUnpark::new(thread::current()));
        loop {
            match self.poll_stream(unpark.clone()) {
                Ok(Async::NotReady) => unpark.park(),
                Ok(Async::Ready(Some(e))) => return Some(Ok(e)),
                Ok(Async::Ready(None)) => return None,
                Err(e) => return Some(Err(e)),
            }
        }
    }
}

impl<T> Spawn<T> {
    fn enter<F, R>(&mut self, unpark: Arc<Unpark>, f: F) -> R
        where F: FnOnce(&mut T, &Task) -> R
    {
        let task = Task {
            id: self.id,
            unpark: unpark,
            events: Events::new(),
        };
        let obj = &mut self.obj;
        f(obj, &task)
    }
}

/// A trait which represents a sink of notifications that a future is ready to
/// make progress.
///
/// This trait is provided as an argument to the `Spawn::poll_future` and
/// `Spawn::poll_stream` functions. It's transitively used as part of the
/// `Task::unpark` method to internally deliver notifications of readiness of a
/// future to move forward.
pub trait Unpark: Send + Sync {
    /// Indicates that an associated future and/or task are ready to make
    /// progress.
    ///
    /// Typically this means that the receiver of the notification should
    /// arrange for the future to get poll'd in a prompt fashion.
    fn unpark(&self);
}

/// A trait representing requests to poll futures.
///
/// This trait is an argument to the `Spawn::execute` which is used to run a
/// future to completion. An executor will receive requests to run a future and
/// an executor is responsible for ensuring that happens in a timely fashion.
pub trait Executor: Send + Sync + 'static {
    /// Requests that `Run` is executed soon on the given executor.
    fn execute(&self, r: Run);
}

struct ThreadUnpark {
    thread: thread::Thread,
    ready: AtomicBool,
}

impl ThreadUnpark {
    fn new(thread: thread::Thread) -> ThreadUnpark {
        ThreadUnpark {
            thread: thread,
            ready: AtomicBool::new(false),
        }
    }

    fn park(&self) {
        if !self.ready.swap(false, Ordering::SeqCst) {
            thread::park();
        }
    }
}

impl Unpark for ThreadUnpark {
    fn unpark(&self) {
        self.ready.store(true, Ordering::SeqCst);
        self.thread.unpark()
    }
}

/// Units of work submitted to an `Executor`, currently only created
/// internally.
pub struct Run {
    spawn: Spawn<BoxFuture<(), ()>>,
    inner: Arc<Inner>,
}

struct Inner {
    mutex: UnparkMutex<Run>,
    exec: Arc<Executor>,
}

impl Run {
    /// Actually run the task (invoking `poll` on its future) on the current
    /// thread.
    pub fn run(self) {
        let Run { mut spawn, inner } = self;

        // SAFETY: the ownership of this `Run` object is evidence that
        // we are in the `POLLING`/`REPOLL` state for the mutex.
        unsafe {
            inner.mutex.start_poll();

            loop {
                match spawn.poll_future(inner.clone()) {
                    Ok(Async::NotReady) => {}
                    Ok(Async::Ready(())) |
                    Err(()) => return inner.mutex.complete(),
                }
                let run = Run { spawn: spawn, inner: inner.clone() };
                match inner.mutex.wait(run) {
                    Ok(()) => return,            // we've waited
                    Err(r) => spawn = r.spawn,   // someone's notified us
                }
            }
        }
    }
}

impl Unpark for Inner {
    fn unpark(&self) {
        match self.mutex.notify() {
            Ok(run) => self.exec.execute(run),
            Err(()) => {}
        }
    }
}

struct IgnoreUnpark;

impl Unpark for IgnoreUnpark {
    fn unpark(&self) {}
}
