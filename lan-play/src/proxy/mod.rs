pub use async_std::net::{ToSocketAddrs, SocketAddr};
pub use self::direct::DirectProxy;

mod direct;

pub mod socket {
    use async_std::io::{self, Read, Write};
    use async_std::net::{ToSocketAddrs, SocketAddr};

    #[async_trait(?Send)]
    pub trait Tcp: Read + Write {
    }

    #[async_trait(?Send)]
    pub trait Udp {
        async fn send_to<A: ToSocketAddrs>(&self, buf: &[u8], addrs: A) -> io::Result<usize>;
        async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)>;
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
