use super::{Proxy, socket, ToSocketAddrs, SocketAddr};
use tokio::task::{self, JoinHandle};
use tokio::net::{TcpStream, UdpSocket};
use tokio::io;

#[async_trait(?Send)]
impl socket::Tcp for TcpStream {

}

#[async_trait(?Send)]
impl socket::Udp for UdpSocket {
    async fn send_to<A: ToSocketAddrs>(&mut self, buf: &[u8], addrs: A) -> io::Result<usize> {
        UdpSocket::send_to(self, buf, addrs).await
    }
    async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        UdpSocket::recv_from(self, buf).await
    }
}


pub struct DirectProxy {
    join: JoinHandle<()>,
}

impl DirectProxy {
    pub fn new() -> Self {
        let join = task::spawn(async {

        });
        Self {
            join
        }
    }
}

#[async_trait(?Send)]
impl Proxy for DirectProxy {
    type Error = io::Error;
    type TcpSocket = TcpStream;
    type UdpSocket = UdpSocket;

    async fn new_tcp<A: ToSocketAddrs>(&mut self, addrs: A) -> io::Result<Self::TcpSocket> {
        TcpStream::connect(addrs).await
    }
    async fn new_udp<A: ToSocketAddrs>(&mut self, addrs: A) -> io::Result<Self::UdpSocket> {
        UdpSocket::bind(addrs).await
    }
    async fn join(&mut self) -> () {
        (&mut self.join).await.unwrap()
    }
}

pub fn new_proxy() -> DirectProxy {
    DirectProxy::new()
}
