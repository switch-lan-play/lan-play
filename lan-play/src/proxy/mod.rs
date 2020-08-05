pub use self::direct::DirectProxy;
pub use std::io;
pub use std::net::{IpAddr, Ipv4Addr, SocketAddr};

mod traits;
mod direct;
#[cfg(feature = "socks5")]
mod socks5;
#[cfg(feature = "socks5")]
pub use self::socks5::Socks5Proxy;
#[cfg(feature = "shadowsocks")]
mod shadowsocks;
#[cfg(feature = "shadowsocks")]
pub use self::shadowsocks::ShadowsocksProxy;

pub use traits::{BoxedProxy, BoxedTcp, BoxedUdp, SendHalf, RecvHalf};
lazy_static! {
    pub static ref ANY_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0);
}

pub mod prelude {
    pub use super::traits::{Udp as _, Tcp as _, Proxy as _};
}

fn io_other(s: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, s)
}

pub fn other<E: Into<Box<dyn std::error::Error + Send + Sync>>>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
}

pub async fn resolve(proxy: &BoxedProxy, dns_server: &SocketAddr, domain: &str) -> io::Result<Vec<Ipv4Addr>> {
    use dns_parser::{Builder, QueryType, QueryClass, Packet, RData, rdata::A};

    let mut builder = Builder::new_query(1, true);
    builder.add_question(domain, false, QueryType::A, QueryClass::IN);
    let p = builder.build()
        .map_err(|_| io_other("Failed to build dns query"))?;
    
    let mut buf = vec![0u8; 8192];
    let mut udp = proxy.new_udp("0.0.0.0:0".parse().unwrap()).await?;
    udp.send_to(&p, &dns_server).await?;
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
        let proxy = DirectProxy::new();
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
        let proxy = DirectProxy::new();
        let mut udp = proxy.new_udp(*ANY_ADDR).await.unwrap();

        let mut buf = [0u8; 8192];
        udp.send_to(b"hello", &target).await?;
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
        let proxy = Socks5Proxy::new(socks5_addr.to_string(), None);
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
