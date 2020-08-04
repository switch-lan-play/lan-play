use std::{net::SocketAddr, sync::{Arc, Mutex as SyncMutex}};
use tokio::io::{
    self, AsyncRead, AsyncWrite,
};
use futures::{future::{poll_fn, Future}, pin_mut};
use std::{pin::Pin, task::{Context, Poll}};

pub type BoxTcp = Box<dyn Tcp + Unpin + Send>;
pub type BoxUdp = Box<dyn Udp + Unpin + Send>;

pub trait Tcp: AsyncRead + AsyncWrite {}

#[async_trait]
pub trait Udp {
    async fn send_to(&mut self, buf: &[u8], addr: SocketAddr) -> io::Result<usize>;
    async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)>;
}
pub trait Udp2 {
    fn poll_send_to(self: &mut Self, cx: &mut Context<'_>, buf: &[u8], target: &SocketAddr) -> Poll<io::Result<usize>>;
    fn poll_recv_from(self: &mut Self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<(usize, SocketAddr)>>;
}

pub async fn send_to<T: Udp2>(s: &mut T, buf: &[u8], target: &SocketAddr) -> io::Result<usize> {
    poll_fn(move |cx| s.poll_send_to(cx, buf, target)).await
}

pub async fn recv_from<T: Udp2>(s: &mut T, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
    poll_fn(move |cx| s.poll_recv_from(cx, buf)).await
}

pub struct SendHalf {
    inner: Arc<SyncMutex<BoxUdp>>,
}
pub struct RecvHalf {
    inner: Arc<SyncMutex<BoxUdp>>,
}

impl dyn Udp + Unpin + Send {
    pub fn split(self: Box<Self>) -> (SendHalf, RecvHalf) {
        let inner = Arc::new(SyncMutex::new(self));
        (SendHalf {
            inner: inner.clone(),
        }, RecvHalf {
            inner,
        })
    }
}

impl SendHalf {
    pub async fn send_to(&mut self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        poll_fn(|cx| {
            let mut inner = self.inner.lock().unwrap();
            let fut = inner.send_to(buf, addr);
            pin_mut!(fut);
            fut.poll(cx)
        }).await
    }
}

impl RecvHalf {
    pub async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        poll_fn(|cx| {
            let mut inner = self.inner.lock().unwrap();
            let fut = inner.recv_from(buf);
            pin_mut!(fut);
            fut.poll(cx)
        }).await
    }
}

pub fn other<E: Into<Box<dyn std::error::Error + Send + Sync>>>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
}