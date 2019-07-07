use core::fmt;
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream, TryStream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project::{pin_project, unsafe_project};

/// Stream for the [`map_ok`](super::TryStreamExt::map_ok) method.
#[unsafe_project(Unpin)]
#[must_use = "streams do nothing unless polled"]
pub struct MapOk<St, F> {
    #[pin]
    stream: St,
    f: F,
}

impl<St, F> fmt::Debug for MapOk<St, F>
where
    St: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapOk")
            .field("stream", &self.stream)
            .finish()
    }
}

impl<St, F> MapOk<St, F> {
    /// Creates a new MapOk.
    pub(super) fn new(stream: St, f: F) -> Self {
        MapOk { stream, f }
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

impl<St: FusedStream, F> FusedStream for MapOk<St, F> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, F, T> Stream for MapOk<St, F>
where
    St: TryStream,
    F: FnMut(St::Ok) -> T,
{
    type Item = Result<T, St::Error>;

    #[pin_project(self)]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        self.stream
            .as_mut()
            .try_poll_next(cx)
            .map(|opt| opt.map(|res| res.map(|x| (self.f)(x))))
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, F, Item> Sink<Item> for MapOk<S, F>
where
    S: Sink<Item>,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}
