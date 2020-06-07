pub use std::io;
pub use std::net::{SocketAddr, IpAddr, Ipv4Addr};
pub use self::direct::DirectProxy;
pub use self::socks5::Socks5Proxy;

mod direct;
mod socks5;
pub type BoxTcp = Box<dyn socket::Tcp + Unpin>;
pub type BoxUdp = Box<dyn socket::Udp + Unpin>;
pub type BoxProxy = Box<dyn Proxy + Unpin>;
lazy_static! {
    pub static ref ANY_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0);
}

pub mod socket {
    use std::net::SocketAddr;
    use tokio::io::{self, AsyncRead, AsyncWrite};

    #[async_trait(?Send)]
    pub trait Tcp: AsyncRead + AsyncWrite {
    }

    #[async_trait(?Send)]
    pub trait Udp {
        async fn send_to(&mut self, buf: &[u8], addr: SocketAddr) -> io::Result<usize>;
        async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)>;
    }
}
#[async_trait(?Send)]
pub trait Proxy
{
    async fn new_tcp(&mut self, addr: SocketAddr) -> io::Result<BoxTcp>;
    async fn new_udp(&mut self, addr: SocketAddr) -> io::Result<BoxUdp>;
    async fn join(&mut self) -> ();
}

#[cfg(test)]
mod test {
    use super::*;
    use tokio::io::{split, copy};
    use tokio::net::{TcpListener, UdpSocket};
    use tokio::prelude::*;
    use super::socks5::test::socks5_server;

    async fn server_tcp() -> (TcpListener, u16) {
        let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = server.local_addr().unwrap().port();
        (server, port)
    }

    async fn server_udp() -> (UdpSocket, u16) {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let port = server.local_addr().unwrap().port();
        (server, port)
    }

    #[tokio::test]
    async fn test_direct_proxy() -> tokio::io::Result<()> {
        let (mut server, port) = server_tcp().await;
        let join = tokio::spawn(async move {
            let (socket, _) = server.accept().await?;
            let (mut reader, mut writer) = split(socket);
            copy(&mut reader, &mut writer).await?;
            Ok::<_, tokio::io::Error>(())
        });
        let mut proxy: BoxProxy = DirectProxy::new();
        let mut tcp = proxy.new_tcp(
            SocketAddr::new("127.0.0.1".parse().unwrap(), port)
        ).await.unwrap();

        let mut buf = [0u8; 5];
        tcp.write_all(b"hello").await?;
        tcp.read_exact(&mut buf).await?;
        assert_eq!(&buf, b"hello");
        tcp.shutdown().await?;

        join.await.unwrap().unwrap();
        Ok(())
    }

    #[tokio::test]
    async fn test_direct_proxy_udp() -> tokio::io::Result<()> {
        let (mut server, port) = server_udp().await;
        let join = tokio::spawn(async move {
            let mut buf = [0u8; 65536];
            let (size, addr) = server.recv_from(&mut buf).await?;
            server.send_to(&buf[..size], addr).await?;
            Ok::<_, tokio::io::Error>(())
        });
        let mut proxy: BoxProxy = DirectProxy::new();
        let mut udp = proxy.new_udp(
            *ANY_ADDR
        ).await.unwrap();
        let target = SocketAddr::new("127.0.0.1".parse().unwrap(), port);

        let mut buf = [0u8; 65536];
        udp.send_to(b"hello", target).await?;
        let (size, addr) = udp.recv_from(&mut buf).await?;
        assert_eq!(addr, target);
        assert_eq!(buf[..size], b"hello"[..]);

        join.await.unwrap().unwrap();
        Ok(())
    }

    #[tokio::test]
    async fn test_socks5_proxy() -> anyhow::Result<()> {
        let (socks5, socks5_port) = socks5_server().await;

        let (mut server, port) = server_tcp().await;
        let join = tokio::spawn(async move {
            let (socket, _) = server.accept().await?;
            let (mut reader, mut writer) = split(socket);
            copy(&mut reader, &mut writer).await?;
            Ok::<_, tokio::io::Error>(())
        });
        let mut proxy: BoxProxy = Socks5Proxy::new(([127, 0, 0, 1], socks5_port).into(), None);
        let mut tcp = proxy.new_tcp(
            SocketAddr::new([127, 0, 0, 1].into(), port)
        ).await.unwrap();

        let mut buf = [0u8; 5];
        tcp.write_all(b"hello").await?;
        tcp.read_exact(&mut buf).await?;
        assert_eq!(&buf, b"hello");
        tcp.shutdown().await?;

        join.await??;
        socks5.await?;
        Ok(())
    }
}
