use tokio::time::{timeout, Duration};
pub use self::direct::DirectProxy;
pub use std::io;
pub use std::net::{IpAddr, Ipv4Addr, SocketAddr};

mod direct;
#[cfg(feature = "socks5")]
mod socks5;
#[cfg(feature = "socks5")]
pub use self::socks5::Socks5Proxy;
#[cfg(feature = "shadowsocks")]
mod shadowsocks;
#[cfg(feature = "shadowsocks")]
pub use self::shadowsocks::ShadowsocksProxy;

pub use socket::{other, BoxTcp, BoxUdp, SendHalf, RecvHalf};
pub type BoxProxy = Box<dyn Proxy + Unpin + Sync + Send>;
lazy_static! {
    pub static ref ANY_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0);
}
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub mod socket {
    use std::{net::SocketAddr, sync::{Arc, Mutex as SyncMutex}};
    use tokio::io::{
        self, AsyncRead, AsyncWrite,
    };
    use futures::{future::{poll_fn, Future}, pin_mut};

    pub type BoxTcp = Box<dyn Tcp + Unpin + Send>;
    pub type BoxUdp = Box<dyn Udp + Unpin + Send>;

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

pub async fn new_tcp_timeout(proxy: &BoxProxy, addr: SocketAddr) -> io::Result<BoxTcp> {
    Ok(timeout(CONNECT_TIMEOUT, proxy.new_tcp(addr)).await??)
}

pub async fn new_udp_timeout(proxy: &BoxProxy, addr: SocketAddr) -> io::Result<BoxUdp> {
    Ok(timeout(CONNECT_TIMEOUT, proxy.new_udp(addr)).await??)
}

fn io_other(s: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, s)
}

pub async fn resolve(proxy: &BoxProxy, dns_server: SocketAddr, domain: &str) -> io::Result<Vec<Ipv4Addr>> {
    use dns_parser::{Builder, QueryType, QueryClass, Packet, RData, rdata::A};

    let mut builder = Builder::new_query(1, true);
    builder.add_question(domain, false, QueryType::A, QueryClass::IN);
    let p = builder.build()
        .map_err(|_| io_other("Failed to build dns query"))?;
    
    let mut buf = vec![0u8; 8192];
    let mut udp = proxy.new_udp("0.0.0.0:0".parse().unwrap()).await?;
    udp.send_to(&p, dns_server).await?;
    let (size, _addr) = udp.recv_from(&mut buf).await?;
    buf.truncate(size);

    let pkt = Packet::parse(&buf)
        .map_err(|_| io_other("Failed to parse dns response"))?;

    let ans = pkt.answers.iter().filter_map(|a| match a.data {
        RData::A(A(ip)) => Some(ip),
        _ => None,
    }).collect::<Vec<_>>();

    Ok(ans)
}

#[derive(Debug)]
pub struct Auth {
    pub username: String,
    pub password: String,
}

#[cfg(test)]
#[cfg(feature = "socks5")]
mod test {
    use super::socks5::test::socks5_server;
    use super::*;
    use tokio::{io::{self, copy, split}, spawn, net::{TcpListener, UdpSocket}, prelude::*};

    async fn server_tcp() -> (TcpListener, SocketAddr) {
        let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();
        (server, addr)
    }

    async fn server_udp() -> (UdpSocket, SocketAddr) {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();
        (server, addr)
    }

    #[tokio::test]
    async fn test_direct_proxy() -> io::Result<()> {
        let (mut server, addr) = server_tcp().await;
        let join = spawn(async move {
            let (socket, _) = server.accept().await?;
            let (mut reader, mut writer) = split(socket);
            copy(&mut reader, &mut writer).await?;
            Ok::<_, io::Error>(())
        });
        let proxy: BoxProxy = DirectProxy::new();
        let mut tcp = proxy
            .new_tcp(addr)
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
    async fn test_direct_proxy_udp() -> io::Result<()> {
        let (mut server, target) = server_udp().await;
        let join = spawn(async move {
            let mut buf = [0u8; 8192];
            let (size, addr) = server.recv_from(&mut buf).await?;
            server.send_to(&buf[..size], addr).await?;
            Ok::<_, io::Error>(())
        });
        let proxy: BoxProxy = DirectProxy::new();
        let mut udp = proxy.new_udp(*ANY_ADDR).await.unwrap();

        let mut buf = [0u8; 8192];
        udp.send_to(b"hello", target).await?;
        let (size, addr) = udp.recv_from(&mut buf).await?;
        assert_eq!(addr, target);
        assert_eq!(buf[..size], b"hello"[..]);

        join.await.unwrap().unwrap();
        Ok(())
    }

    #[tokio::test]
    async fn test_socks5_proxy() -> anyhow::Result<()> {
        let (socks5, socks5_addr) = socks5_server().await;

        let (mut server, addr) = server_tcp().await;
        let join = spawn(async move {
            let (socket, _) = server.accept().await?;
            let (mut reader, mut writer) = split(socket);
            copy(&mut reader, &mut writer).await?;
            Ok::<_, io::Error>(())
        });
        let proxy: BoxProxy = Socks5Proxy::new(socks5_addr.to_string(), None);
        let mut tcp = proxy
            .new_tcp(addr)
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
