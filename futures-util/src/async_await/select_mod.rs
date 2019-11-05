//! The `select` macro.

use proc_macro_hack::proc_macro_hack;

#[doc(hidden)]
#[macro_export]
macro_rules! document_select_macro {
    ($item:item) => {
        /// Polls multiple futures and streams simultaneously, executing the branch
        /// for the future that finishes first. If multiple futures are ready,
        /// one will be pseudo-randomly selected at runtime. Futures directly
        /// passed to `select!` must be `Unpin` and implement `FusedFuture`.
        ///
        /// If an expression which yields a `Future` is passed to `select!`
        /// (e.g. an `async fn` call) instead of a `Future` by name the `Unpin`
        /// requirement is relaxed, since the macro will pin the resulting `Future`
        /// on the stack. However the `Future` returned by the expression must
        /// still implement `FusedFuture`. This difference is presented
        ///
        /// Futures and streams which are not already fused can be fused using the
        /// `.fuse()` method. Note, though, that fusing a future or stream directly
        /// in the call to `select!` will not be enough to prevent it from being
        /// polled after completion if the `select!` call is in a loop, so when
        /// `select!`ing in a loop, users should take care to `fuse()` outside of
        /// the loop.
        ///
        /// `select!` can be used as an expression and will return the return
        /// value of the selected branch. For this reason the return type of every
        /// branch in a `select!` must be the same.
        ///
        /// This macro is only usable inside of async functions, closures, and blocks.
        /// It is also gated behind the `async-await` feature of this library, which is
        /// _not_ activated by default.
        ///
        /// Note that `select!` relies on `proc-macro-hack`, and may require to set the
        /// compiler's recursion limit very high, e.g. `#![recursion_limit="1024"]`.
        ///
        /// # Examples
        ///
        /// ```
        /// # futures::executor::block_on(async {
        /// use futures::future;
        /// use futures::select;
        /// let mut a = future::ready(4);
        /// let mut b = future::pending::<()>();
        ///
        /// let res = select! {
        ///     a_res = a => a_res + 1,
        ///     _ = b => 0,
        /// };
        /// assert_eq!(res, 5);
        /// # });
        /// ```
        ///
        /// ```
        /// # futures::executor::block_on(async {
        /// use futures::future;
        /// use futures::stream::{self, StreamExt};
        /// use futures::select;
        /// let mut st = stream::iter(vec![2]).fuse();
        /// let mut fut = future::pending::<()>();
        ///
        /// select! {
        ///     x = st.next() => assert_eq!(Some(2), x),
        ///     _ = fut => panic!(),
        /// };
        /// # });
        /// ```
        ///
        /// As described earlier, `select` can directly select on expressions
        /// which return `Future`s - even if those do not implement `Unpin`:
        ///
        /// ```
        /// # futures::executor::block_on(async {
        /// use futures::future::FutureExt;
        /// use futures::select;
        ///
        /// // Calling the following async fn returns a Future which does not
        /// // implement Unpin
        /// async fn async_identity_fn(arg: usize) -> usize {
        ///     arg
        /// }
        ///
        /// let res = select! {
        ///     a_res = async_identity_fn(62).fuse() => a_res + 1,
        ///     b_res = async_identity_fn(13).fuse() => b_res,
        /// };
        /// assert!(res == 63 || res == 12);
        /// # });
        /// ```
        ///
        /// If a similar async function is called outside of `select` to produce
        /// a `Future`, the `Future` must be pinned in order to be able to pass
        /// it to `select`. This can be achieved via `Box::pin` for pinning a
        /// `Future` on the heap or the `pin_mut!` macro for pinning a `Future`
        /// on the stack.
        ///
        /// ```
        /// # futures::executor::block_on(async {
        /// use futures::future::FutureExt;
        /// use futures::select;
        /// use pin_utils::pin_mut;
        ///
        /// // Calling the following async fn returns a Future which does not
        /// // implement Unpin
        /// async fn async_identity_fn(arg: usize) -> usize {
        ///     arg
        /// }
        ///
        /// let fut_1 = async_identity_fn(1).fuse();
        /// let fut_2 = async_identity_fn(2).fuse();
        /// let mut fut_1 = Box::pin(fut_1); // Pins the Future on the heap
        /// pin_mut!(fut_2); // Pins the Future on the stack
        ///
        /// let res = select! {
        ///     a_res = fut_1 => a_res,
        ///     b_res = fut_2 => b_res,
        /// };
        /// assert!(res == 1 || res == 2);
        /// # });
        /// ```
        ///
        /// `select` also accepts a `complete` branch and a `default` branch.
        /// `complete` will run if all futures and streams have already been
        /// exhausted. `default` will run if no futures or streams are
        /// immediately ready. `complete` takes priority over `default` in
        /// the case where all futures have completed.
        /// A motivating use-case for passing `Future`s by name as well as for
        /// `complete` blocks is to call `select!` in a loop, which is
        /// demonstrated in the following example:
        ///
        /// ```
        /// # futures::executor::block_on(async {
        /// use futures::future;
        /// use futures::select;
        /// let mut a_fut = future::ready(4);
        /// let mut b_fut = future::ready(6);
        /// let mut total = 0;
        ///
        /// loop {
        ///     select! {
        ///         a = a_fut => total += a,
        ///         b = b_fut => total += b,
        ///         complete => break,
        ///         default => panic!(), // never runs (futures run first, then complete)
        ///     };
        /// }
        /// assert_eq!(total, 10);
        /// # });
        /// ```
        ///
        /// Note that the futures that have been matched over can still be mutated
        /// from inside the `select!` block's branches. This can be used to implement
        /// more complex behavior such as timer resets or writing into the head of
        /// a stream.
        $item
    }
}

document_select_macro! {
    #[proc_macro_hack(support_nested)]
    pub use futures_macro::select;
}
