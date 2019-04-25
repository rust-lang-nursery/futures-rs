#![feature(async_await, await_macro)]

use futures::future;
use futures::FutureExt;
use futures::task::Poll;
use futures::stream::FusedStream;
use futures::stream::{SelectAll, StreamExt};
use futures_test::task::noop_context;

#[test]
fn is_terminated() {
    let mut cx = noop_context();
    let mut tasks = SelectAll::new();

    assert_eq!(tasks.is_terminated(), false);
    assert_eq!(tasks.poll_next_unpin(&mut cx), Poll::Ready(None));
    assert_eq!(tasks.is_terminated(), true);

    // Test that the sentinel value doesn't leak
    assert_eq!(tasks.is_empty(), true);
    assert_eq!(tasks.len(), 0);

    tasks.push(future::ready(1).into_stream());

    assert_eq!(tasks.is_empty(), false);
    assert_eq!(tasks.len(), 1);

    assert_eq!(tasks.is_terminated(), false);
    assert_eq!(tasks.poll_next_unpin(&mut cx), Poll::Ready(Some(1)));
    assert_eq!(tasks.is_terminated(), false);
    assert_eq!(tasks.poll_next_unpin(&mut cx), Poll::Ready(None));
    assert_eq!(tasks.is_terminated(), true);
}
