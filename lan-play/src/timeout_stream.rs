use crate::rt::{AsyncRead, AsyncWrite, Duration, Instant, io, Delay, delay_for};
use std::task::{Context, Poll};
use std::pin::Pin;
use std::future::Future;
use pin_project_lite::pin_project;
use futures::ready;

pin_project! {
    pub struct TimeoutStream<S>
    {
        #[pin]
        s: S,
        last_visit: Instant,
        timeout: Duration,
        timer: Delay,
    }
}

impl<S> TimeoutStream<S>
{
    pub fn new(s: S, timeout: Duration) -> TimeoutStream<S>
    where
        S: AsyncRead + AsyncWrite,
    {
        TimeoutStream {
            s,
            last_visit: Instant::now(),
            timeout,
            timer: delay_for(timeout),
        }
    }
    fn timeout(&self) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::TimedOut, "Timedout"))
    }
    fn poll_timeout(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            ready!(Pin::new(&mut self.timer).poll(cx));
            let elapsed = self.last_visit.elapsed();
            log::trace!("e {:?}", elapsed);
            if elapsed > self.timeout {
                return Poll::Ready(self.timeout())
            } else {
                self.timer = delay_for(self.timeout - elapsed);
            }
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
        match self.as_mut().project().s.poll_read(cx, buf) {
            Poll::Ready(r) => {
                self.last_visit = Instant::now();
                Poll::Ready(r)
            }
            Poll::Pending => {
                ready!(self.poll_timeout(cx))?;
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
        match self.as_mut().project().s.poll_write(cx, buf) {
            Poll::Ready(r) => {
                self.last_visit = Instant::now();
                Poll::Ready(r)
            }
            Poll::Pending => {
                ready!(self.poll_timeout(cx))?;
                unreachable!()
            }
        }
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        this.s.poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        this.s.poll_shutdown(cx)
    }
}
