use alloc::sync::Arc;

/// A way of waking up a specific task.
///
/// By implementing this trait, types that are expected to be wrapped in an `Arc`
/// can be converted into `Waker` objects.
/// Those Wakers can be used to signal executors that a task it owns
/// is ready to be `poll`ed again.
// Note: Send + Sync required because `Arc<T>` doesn't automatically imply
// those bounds, but `Waker` implements them.
pub trait ArcWake: Send + Sync {
    /// Indicates that the associated task is ready to make progress and should
    /// be `poll`ed.
    ///
    /// This function can be called from an arbitrary thread, including threads which
    /// did not create the `ArcWake` based `Waker`.
    ///
    /// Executors generally maintain a queue of "ready" tasks; `wake` should place
    /// the associated task onto this queue.
    fn wake(self: Arc<Self>) {
        Self::wake_by_ref(&self)
    }

    /// Indicates that the associated task is ready to make progress and should
    /// be `poll`ed.
    ///
    /// This function can be called from an arbitrary thread, including threads which
    /// did not create the `ArcWake` based `Waker`.
    ///
    /// Executors generally maintain a queue of "ready" tasks; `wake_by_ref` should place
    /// the associated task onto this queue.
    ///
    /// This function is similar to `wake`, but must not consume the provided data
    /// pointer.
    fn wake_by_ref(arc_self: &Arc<Self>);
}
