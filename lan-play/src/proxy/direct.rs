use super::{traits, BoxedProxy, BoxedTcp, BoxedUdp, SocketAddr, prelude::*};
use tokio::{io, net::{TcpStream, UdpSocket}};
use std::task::{Context, Poll};
use futures::ready;

impl traits::Tcp for TcpStream {}

impl traits::Udp for UdpSocket {
    fn poll_send_to(self: &mut Self, cx: &mut Context<'_>, buf: &[u8], target: &SocketAddr) -> Poll<io::Result<usize>> {
        UdpSocket::poll_send_to(&self, cx, buf, target)
    }
    fn poll_recv_from(self: &mut Self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<(usize, SocketAddr)>> {
        let mut buf = io::ReadBuf::new(buf);
        let addr = ready!(UdpSocket::poll_recv_from(&self, cx, &mut buf))?;
        Poll::Ready(Ok((buf.filled().len(), addr)))
    }
}

pub struct DirectProxy {}

impl DirectProxy {
    pub fn new() -> BoxedProxy {
        Self {}.boxed()
    }
}

#[async_trait]
impl traits::Proxy for DirectProxy {
    async fn new_tcp(&self, addr: SocketAddr) -> io::Result<BoxedTcp> {
        Ok(TcpStream::connect(addr).await?.boxed())
    }
    async fn new_udp(&self, addr: SocketAddr) -> io::Result<BoxedUdp> {
        Ok(UdpSocket::bind(addr).await?.boxed())
    }
}
