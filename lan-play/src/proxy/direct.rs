use super::{socket, BoxProxy, BoxTcp, BoxUdp, Proxy, SocketAddr};
use crate::rt::{io, TcpStream, UdpSocket};

impl socket::Tcp for TcpStream {}

#[async_trait]
impl socket::Udp for UdpSocket {
    async fn send_to(&mut self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        UdpSocket::send_to(self, buf, addr).await
    }
    async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        UdpSocket::recv_from(self, buf).await
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
