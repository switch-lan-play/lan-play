use std::{net::SocketAddr, sync::{Arc, Mutex as SyncMutex}};
use tokio::{io::{
    self, AsyncRead, AsyncWrite,
    
}, time::{timeout, Duration}};
use futures::future::poll_fn;
use std::task::{Context, Poll};

pub type BoxedTcp = Box<dyn Tcp + Unpin + Send + Sync>;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[async_trait]
pub trait Proxy {
    async fn new_tcp(&self, addr: SocketAddr) -> io::Result<BoxedTcp>;
    async fn new_udp(&self, addr: SocketAddr) -> io::Result<BoxedUdp>;
    fn boxed(self) -> BoxedProxy
    where
        Self: Sized + Unpin + Send + Sync + 'static,
    {
        BoxedProxy(Box::new(self))
    }
}

pub struct BoxedProxy(pub(super) Box<dyn Proxy + Unpin + Send + Sync>);

impl BoxedProxy {
    pub async fn new_tcp(&self, addr: SocketAddr) -> io::Result<BoxedTcp> {
        self.0.new_tcp(addr).await
    }
    pub async fn new_udp(&self, addr: SocketAddr) -> io::Result<BoxedUdp> {
        self.0.new_udp(addr).await
    }
    pub async fn new_tcp_timeout(&self, addr: SocketAddr) -> io::Result<BoxedTcp> {
        Ok(timeout(CONNECT_TIMEOUT, self.new_tcp(addr)).await??)
    }
    pub async fn new_udp_timeout(&self, addr: SocketAddr) -> io::Result<BoxedUdp> {
        Ok(timeout(CONNECT_TIMEOUT, self.new_udp(addr)).await??)
    }
}

pub trait Tcp: AsyncRead + AsyncWrite {
    fn boxed(self) -> BoxedTcp
    where
        Self: Sized + Unpin + Send + Sync + 'static,
    {
        Box::new(self)
    }
}

pub trait Udp {
    fn poll_send_to(self: &mut Self, cx: &mut Context<'_>, buf: &[u8], target: &SocketAddr) -> Poll<io::Result<usize>>;
    fn poll_recv_from(self: &mut Self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<(usize, SocketAddr)>>;
    fn boxed(self) -> BoxedUdp
    where
        Self: Sized + Unpin + Send + Sync + 'static,
    {
        BoxedUdp(Box::new(self))
    }
}

pub struct BoxedUdp(pub(super) Box<dyn Udp + Unpin + Send + Sync>);

impl BoxedUdp {
    pub async fn send_to(&mut self, buf: &[u8], target: &SocketAddr) -> io::Result<usize> {
        poll_fn(|cx| self.0.poll_send_to(cx, buf, target)).await
    }
    pub async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        poll_fn(|cx| self.0.poll_recv_from(cx, buf)).await
    }
    pub fn split(self) -> (SendHalf, RecvHalf) {
        let inner = Arc::new(SyncMutex::new(self));
        (SendHalf {
            inner: inner.clone(),
        }, RecvHalf {
            inner,
        })
    }
}

pub struct SendHalf {
    inner: Arc<SyncMutex<BoxedUdp>>,
}
pub struct RecvHalf {
    inner: Arc<SyncMutex<BoxedUdp>>,
}

impl SendHalf {
    pub async fn send_to(&mut self, buf: &[u8], addr: &SocketAddr) -> io::Result<usize> {
        poll_fn(|cx| {
            let mut inner = self.inner.lock().unwrap();
            inner.0.poll_send_to(cx, buf, addr)
        }).await
    }
}

impl RecvHalf {
    pub async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        poll_fn(|cx| {
            let mut inner = self.inner.lock().unwrap();
            inner.0.poll_recv_from(cx, buf)
        }).await
    }
}
