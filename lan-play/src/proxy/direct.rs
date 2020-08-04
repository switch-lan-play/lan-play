use super::{traits, BoxProxy, BoxTcp, BoxUdp, Proxy, SocketAddr};
use tokio::{io, net::{TcpStream, UdpSocket}};
use std::{pin::Pin, task::{Context, Poll}};

impl traits::Tcp for TcpStream {}

#[async_trait]
impl traits::Udp for UdpSocket {
    async fn send_to(&mut self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        UdpSocket::send_to(self, buf, addr).await
    }
    async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        UdpSocket::recv_from(self, buf).await
    }
}

impl traits::Udp2 for UdpSocket {
    fn poll_send_to(self: &mut Self, cx: &mut Context<'_>, buf: &[u8], target: &SocketAddr) -> Poll<io::Result<usize>> {
        UdpSocket::poll_send_to(&self, cx, buf, target)
    }
    fn poll_recv_from(self: &mut Self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<(usize, SocketAddr)>> {
        UdpSocket::poll_recv_from(&self, cx, buf)
    }
}

pub struct DirectProxy {}

impl DirectProxy {
    pub fn new() -> BoxProxy {
        Box::new(Self {})
    }
}

#[async_trait]
impl Proxy for DirectProxy {
    async fn new_tcp(&self, addr: SocketAddr) -> io::Result<BoxTcp> {
        Ok(Box::new(TcpStream::connect(addr).await?))
    }
    async fn new_udp(&self, addr: SocketAddr) -> io::Result<BoxUdp> {
        Ok(Box::new(UdpSocket::bind(addr).await?))
    }
}
