use async_channel::{Sender, TrySendError};
use futures::Sink;
use std::{pin::Pin, task::{Context, Poll}};

/// Only for unbouned Sender, poll_ready is not handled
#[derive(Debug, Clone)]
pub struct SinkSender<T>(pub Sender<T>);

impl<T> Sink<T> for SinkSender<T> {
    type Error = TrySendError<T>;
    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        self.0.try_send(item)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.close();
        Poll::Ready(Ok(()))
    }
}
