pub use std::io;
pub use std::net::{SocketAddr, IpAddr, Ipv4Addr};
pub use self::direct::DirectProxy;
pub use self::socks5::Socks5Proxy;

mod direct;
mod socks5;
pub use socket::{BoxTcp, BoxUdp, Udp2, other};
pub type BoxProxy = Box<dyn Proxy + Unpin + Sync + Send>;
lazy_static! {
    pub static ref ANY_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0);
}

pub mod socket {
    use std::{net::SocketAddr, sync::Arc};
    use tokio::{io::{self, AsyncRead, AsyncWrite}, sync::{Notify, Mutex}};

    pub type BoxTcp = Box<dyn Tcp + Unpin + Send>;
    pub type BoxUdp = Box<dyn Udp + Unpin + Send>;
    #[async_trait]
    pub trait Tcp: AsyncRead + AsyncWrite {
    }

    #[async_trait]
    pub trait Udp {
        async fn send_to(&mut self, buf: &[u8], addr: SocketAddr) -> io::Result<usize>;
        async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)>;
    }

    pub struct Udp2(Arc<(Mutex<BoxUdp>, Notify, Notify)>);

    impl From<BoxUdp> for Udp2 {
        fn from(u: BoxUdp) -> Self {
            Udp2::new(u)
        }
    }

    impl Udp2 {
        fn new(u: BoxUdp) -> Self {
            let inner = Arc::new((
                Mutex::new(u), Notify::new(), Notify::new()
            ));
            Udp2(inner)
        }
        pub async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
            let inner = &self.0;
            let udp = &inner.0;
            let notify1 = &inner.1;
            let notify2 = &inner.2;
            notify1.notify();
            let r = udp.lock().await.send_to(buf, addr).await;
            notify2.notify();
            r
        }
        pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
            let inner = &self.0;
            let udp = &inner.0;
            let notify1 = &inner.1;
            let notify2 = &inner.2;

            loop {
                let mut lock = udp.lock().await;
                tokio::select! {
                    r = lock.recv_from(buf) => {
                        return r
                    }
                    _ = notify1.notified() => {
                        drop(lock);
                        notify2.notified().await;
                    }
                };
            }
        }
    }
    pub fn other<E: Into<Box<dyn std::error::Error + Send + Sync>>>(e: E) -> io::Error {
        io::Error::new(io::ErrorKind::Other, e)
    }
}
#[async_trait]
pub trait Proxy
{
    async fn new_tcp(&self, addr: SocketAddr) -> io::Result<BoxTcp>;
    async fn new_udp(&self, addr: SocketAddr) -> io::Result<BoxUdp>;
}

pub fn spawn_udp(mut udp: BoxUdp) {
    tokio::spawn(async move {
        let mut buf = vec![0u8; 1000];
        let (size, addr) = udp.recv_from(&mut buf).await?;
        udp.send_to(&buf, addr).await?;
        Ok::<_, io::Error>(())
    });
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
        let proxy: BoxProxy = DirectProxy::new();
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
        let proxy: BoxProxy = DirectProxy::new();
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
        let proxy: BoxProxy = Socks5Proxy::new(([127, 0, 0, 1], socks5_port).into(), None);
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
