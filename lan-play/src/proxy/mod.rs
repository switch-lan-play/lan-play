pub use std::net::SocketAddr;
pub use tokio::net::ToSocketAddrs;
pub use self::direct::DirectProxy;

mod direct;

pub mod socket {
    use std::net::SocketAddr;
    use tokio::io::{self, AsyncRead, AsyncWrite};
    use tokio::net::{ToSocketAddrs};

    #[async_trait(?Send)]
    pub trait Tcp: AsyncRead + AsyncWrite {
    }

    #[async_trait(?Send)]
    pub trait Udp {
        async fn send_to<A: ToSocketAddrs>(&mut self, buf: &[u8], addrs: A) -> io::Result<usize>;
        async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)>;
    }
}

#[async_trait(?Send)]
pub trait Proxy
{
    type Error;
    type TcpSocket: socket::Tcp;
    type UdpSocket: socket::Udp;

    async fn new_tcp<A: ToSocketAddrs>(&mut self, addrs: A) -> Result<Self::TcpSocket, Self::Error>;
    async fn new_udp<A: ToSocketAddrs>(&mut self, addrs: A) -> Result<Self::UdpSocket, Self::Error>;
    async fn join(&mut self) -> ();
}
