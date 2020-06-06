use super::{Proxy, socket, SocketAddr, BoxTcp, BoxUdp, BoxProxy};
use tokio::task::{self, JoinHandle};
use tokio::net::{TcpStream, UdpSocket};
use tokio::io;

#[async_trait(?Send)]
impl socket::Tcp for TcpStream {

}

#[async_trait(?Send)]
impl socket::Udp for UdpSocket {
    async fn send_to(&mut self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        UdpSocket::send_to(self, buf, addr).await
    }
    async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        UdpSocket::recv_from(self, buf).await
    }
}


pub struct DirectProxy {
    join: JoinHandle<()>,
}

impl DirectProxy {
    pub fn new() -> BoxProxy {
        let join = task::spawn(async {

        });
        Box::new(Self {
            join
        })
    }
}

#[async_trait(?Send)]
impl Proxy for DirectProxy {
    async fn new_tcp(&mut self, addr: SocketAddr) -> io::Result<BoxTcp> {
        Ok(Box::new(TcpStream::connect(addr).await?))
    }
    async fn new_udp(&mut self, addr: SocketAddr) -> io::Result<BoxUdp> {
        Ok(Box::new(UdpSocket::bind(addr).await?))
    }
    async fn join(&mut self) -> () {
        (&mut self.join).await.unwrap()
    }
}
