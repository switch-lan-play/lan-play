use crate::rt::{AsyncRead, AsyncWrite, Duration, io};
use std::task::{Context, Poll};
use std::pin::Pin;
use futures::ready;
use async_timeout::Timeout;

#[derive(Debug)]
pub struct TimeoutStream<S>
{
    s: S,
    timeout: Timeout,
}

impl<S> TimeoutStream<S>
{
    pub fn new(s: S, timeout: Duration) -> TimeoutStream<S>
    where
        S: AsyncRead + AsyncWrite,
    {
        TimeoutStream {
            s,
            timeout: Timeout::new(timeout),
        }
    }
}

impl<S> AsyncRead for TimeoutStream<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        match Pin::new(&mut self.s).poll_read(cx, buf) {
            Poll::Ready(r) => {
                self.timeout.visit();
                Poll::Ready(r)
            }
            Poll::Pending => {
                ready!(self.timeout.poll_timeout(cx))?;
                unreachable!()
            }
        }
    }
}

impl<S> AsyncWrite for TimeoutStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        match Pin::new(&mut self.s).poll_write(cx, buf) {
            Poll::Ready(r) => {
                self.timeout.visit();
                Poll::Ready(r)
            }
            Poll::Pending => {
                ready!(self.timeout.poll_timeout(cx))?;
                unreachable!()
            }
        }
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.s).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.s).poll_shutdown(cx)
    }
}
