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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio::prelude::*;

    async fn server() -> u16 {
        let mut server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = server.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut socket, _) = server.accept().await.unwrap();
            socket.write_all(b"hi").await.unwrap();
        });
        port
    }

    #[tokio::test]
    async fn test_direct_proxy() {
        let port = server().await;
        let mut proxy = DirectProxy::new();
        let mut tcp = proxy.new_tcp(("127.0.0.1", port)).await.unwrap();
        let mut buf = [0u8; 2];
        tcp.read_exact(&mut buf).await.unwrap();
        // assert!(&buf, b"hi");
        println!("{:?}", buf);
    }
}
