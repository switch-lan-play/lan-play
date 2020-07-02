pub use self::direct::DirectProxy;
pub use self::socks5::Socks5Proxy;
pub use std::io;
pub use std::net::{IpAddr, Ipv4Addr, SocketAddr};

mod direct;
mod socks5;
pub use socket::{other, BoxTcp, BoxUdp, SendHalf, RecvHalf};
pub type BoxProxy = Box<dyn Proxy + Unpin + Sync + Send>;
lazy_static! {
    pub static ref ANY_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0);
}

pub mod socket {
    use std::{net::SocketAddr, sync::{Arc, Mutex as SyncMutex}};
    use tokio::{
        io::{self, AsyncRead, AsyncWrite},
    };
    use futures::{future::{poll_fn, Future}, pin_mut};

    pub type BoxTcp = Box<dyn Tcp + Unpin + Send>;
    pub type BoxUdp = Box<dyn Udp + Unpin + Send>;
    #[async_trait]
    pub trait Tcp: AsyncRead + AsyncWrite {}

    #[async_trait]
    pub trait Udp {
        async fn send_to(&mut self, buf: &[u8], addr: SocketAddr) -> io::Result<usize>;
        async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)>;
    }

    pub struct SendHalf {
        inner: Arc<SyncMutex<BoxUdp>>,
    }
    pub struct RecvHalf {
        inner: Arc<SyncMutex<BoxUdp>>,
    }

    impl dyn Udp + Unpin + Send {
        pub fn split(self: Box<Self>) -> (SendHalf, RecvHalf) {
            let inner = Arc::new(SyncMutex::new(self));
            (SendHalf {
                inner: inner.clone(),
            }, RecvHalf {
                inner,
            })
        }
    }

    impl SendHalf {
        pub async fn send_to(&mut self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
            poll_fn(|cx| {
                let mut inner = self.inner.lock().unwrap();
                let fut = inner.send_to(buf, addr);
                pin_mut!(fut);
                fut.poll(cx)
            }).await
        }
    }

    impl RecvHalf {
        pub async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
            poll_fn(|cx| {
                let mut inner = self.inner.lock().unwrap();
                let fut = inner.recv_from(buf);
                pin_mut!(fut);
                fut.poll(cx)
            }).await
        }
    }

    pub fn other<E: Into<Box<dyn std::error::Error + Send + Sync>>>(e: E) -> io::Error {
        io::Error::new(io::ErrorKind::Other, e)
    }
}
#[async_trait]
pub trait Proxy {
    async fn new_tcp(&self, addr: SocketAddr) -> io::Result<BoxTcp>;
    async fn new_udp(&self, addr: SocketAddr) -> io::Result<BoxUdp>;
}

#[derive(Debug)]
pub struct Auth {
    pub username: String,
    pub password: String,
}

#[cfg(test)]
mod test {
    use super::socks5::test::socks5_server;
    use super::*;
    use tokio::io::{copy, split};
    use tokio::net::{TcpListener, UdpSocket};
    use tokio::prelude::*;

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
        let mut tcp = proxy
            .new_tcp(SocketAddr::new("127.0.0.1".parse().unwrap(), port))
            .await
            .unwrap();

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
        let mut udp = proxy.new_udp(*ANY_ADDR).await.unwrap();
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
        let proxy: BoxProxy = Socks5Proxy::new(format!("127.0.0.1:{}", socks5_port), None);
        let mut tcp = proxy
            .new_tcp(SocketAddr::new([127, 0, 0, 1].into(), port))
            .await
            .unwrap();

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
