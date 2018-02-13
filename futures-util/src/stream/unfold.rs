use core::mem;

use futures_core::{Future, IntoFuture, Async, Poll, Stream};
use futures_core::task;

/// Creates a `Stream` from a seed and a closure returning a `Future`.
///
/// This function is the dual for the `Stream::fold()` adapter: while
/// `Stream::fold()` reduces a `Stream` to one single value, `unfold()` creates a
/// `Stream` from a seed value.
///
/// `unfold()` will call the provided closure with the provided seed, then wait
/// for the returned `Future` to complete with `(a, b)`. It will then yield the
/// value `a`, and use `b` as the next internal state.
///
/// If the closure returns `None` instead of `Some(Future)`, then the `unfold()`
/// will stop producing items and return `Ok(Async::Ready(None))` in future
/// calls to `poll()`.
///
/// In case of error generated by the returned `Future`, the error will be
/// returned by the `Stream`.  The `Stream` will then yield
/// `Ok(Async::Ready(None))` in future calls to `poll()`.
///
/// This function can typically be used when wanting to go from the "world of
/// futures" to the "world of streams": the provided closure can build a
/// `Future` using other library functions working on futures, and `unfold()`
/// will turn it into a `Stream` by repeating the operation.
///
/// # Example
///
/// ```rust
/// # extern crate futures;
/// # extern crate futures_executor;
///
/// use futures::prelude::*;
/// use futures::stream;
/// use futures::future;
/// use futures_executor::current_thread::run;
///
/// # fn main() {
/// let mut stream = stream::unfold(0, |state| {
///     if state <= 2 {
///         let next_state = state + 1;
///         let yielded = state  * 2;
///         let fut = future::ok::<_, u32>((yielded, next_state));
///         Some(fut)
///     } else {
///         None
///     }
/// });
///
/// let result = run(|c| c.block_on(stream.collect()));
/// assert_eq!(result, Ok(vec![0, 2, 4]));
/// # }
/// ```
pub fn unfold<T, F, Fut, It>(init: T, f: F) -> Unfold<T, F, Fut>
    where F: FnMut(T) -> Fut,
          Fut: IntoFuture<Item = Option<(It, T)>>,
{
    Unfold {
        f: f,
        state: State::Ready(init),
    }
}

/// A stream which creates futures, polls them and return their result
///
/// This stream is returned by the `futures::stream::unfold` method
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Unfold<T, F, Fut> where Fut: IntoFuture {
    f: F,
    state: State<T, Fut::Future>,
}

impl <T, F, Fut, It> Stream for Unfold<T, F, Fut>
    where F: FnMut(T) -> Fut,
          Fut: IntoFuture<Item = Option<(It, T)>>,
{
    type Item = It;
    type Error = Fut::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<Option<It>, Fut::Error> {
        loop {
            match mem::replace(&mut self.state, State::Empty) {
                // State::Empty may happen if the future returned an error
                State::Empty => { return Ok(Async::Ready(None)); }
                State::Ready(state) => {
                    self.state = State::Processing((self.f)(state).into_future());
                }
                State::Processing(mut fut) => {
                    match fut.poll(cx)? {
                        Async:: Ready(Some((item, next_state))) => {
                            self.state = State::Ready(next_state);
                            return Ok(Async::Ready(Some(item)));
                        }
                        Async:: Ready(None) => {
                            return Ok(Async::Ready(None))
                        }
                        Async::Pending => {
                            self.state = State::Processing(fut);
                            return Ok(Async::Pending);
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
enum State<T, F> where F: Future {
    /// Placeholder state when doing work, or when the returned Future generated an error
    Empty,

    /// Ready to generate new future; current internal state is the `T`
    Ready(T),

    /// Working on a future generated previously
    Processing(F),
}
