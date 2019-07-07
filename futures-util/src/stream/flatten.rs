use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project::{pin_project, unsafe_project};

/// Stream for the [`flatten`](super::StreamExt::flatten) method.
#[unsafe_project(Unpin)]
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Flatten<St>
    where St: Stream,
{
    #[pin]
    stream: St,
    #[pin]
    next: Option<St::Item>,
}

impl<St: Stream> Flatten<St>
where St: Stream,
      St::Item: Stream,
{
    pub(super) fn new(stream: St) -> Flatten<St>{
        Flatten { stream, next: None, }
    }

    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &St {
        &self.stream
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut St {
        &mut self.stream
    }

    /// Acquires a pinned mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    #[pin_project(self)]
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut St> {
        self.stream
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<St: Stream + FusedStream> FusedStream for Flatten<St> {
    fn is_terminated(&self) -> bool {
        self.next.is_none() && self.stream.is_terminated()
    }
}

impl<St> Stream for Flatten<St>
    where St: Stream,
          St::Item: Stream,
{
    type Item = <St::Item as Stream>::Item;

    #[pin_project(self)]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        loop {
            if self.next.is_none() {
                match ready!(self.stream.as_mut().poll_next(cx)) {
                    Some(e) => self.next.set(Some(e)),
                    None => return Poll::Ready(None),
                }
            }
            let item = ready!(self.next.as_mut().as_pin_mut().unwrap().poll_next(cx));
            if item.is_some() {
                return Poll::Ready(item);
            } else {
                self.next.set(None);
            }
        }
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for Flatten<S>
    where S: Stream + Sink<Item>,
          S::Item: Stream,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
