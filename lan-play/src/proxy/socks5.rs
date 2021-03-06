use tokio::{net::{TcpStream, UdpSocket}, io::{self, BufWriter}};
use super::{other, traits, BoxedProxy, BoxedTcp, BoxedUdp, SocketAddr, Auth, prelude::*};
use async_socks5::{connect, AddrKind, SocksDatagram};
use std::task::{Context, Poll};
use futures::{pin_mut, future::Future, ready};

impl traits::Udp for SocksDatagram<BufWriter<TcpStream>> {
    fn poll_send_to(self: &mut Self, cx: &mut Context<'_>, buf: &[u8], target: &SocketAddr) -> Poll<io::Result<usize>> {
        let fut = self.send_to(buf, *target);
        pin_mut!(fut);
        let r = ready!(fut.poll(cx));
        Poll::Ready(r.map_err(other))
    }
    fn poll_recv_from(self: &mut Self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<(usize, SocketAddr)>> {
        let fut = self.recv_from(buf);
        pin_mut!(fut);
        let r = ready!(fut.poll(cx));
        let r = match r {
            Ok((size, AddrKind::Ip(addr))) => Ok((size, addr)),
            Ok((_, AddrKind::Domain(domain, port))) => {
                let err = format!("Socks5 udp recv_from get domain {}:{}", domain, port);
                Err(other(err))
            },
            Err(e) => Err(other(e)),
        };
        Poll::Ready(r)
    }
}

pub struct Socks5Proxy {
    server: String,
    auth: Option<async_socks5::Auth>,
}

impl Socks5Proxy {
    pub fn new(server: String, auth: Option<Auth>) -> BoxedProxy {
        Self {
            server,
            auth: auth.map(|a| async_socks5::Auth::new(a.username, a.password))
        }.boxed()
    }
}

#[async_trait]
impl traits::Proxy for Socks5Proxy {
    async fn new_tcp(&self, addr: SocketAddr) -> io::Result<BoxedTcp> {
        let mut socket = TcpStream::connect(&self.server).await?;
        connect(&mut socket, addr, self.auth.clone())
            .await
            .map_err(other)?;

        Ok(socket.boxed())
    }
    async fn new_udp(&self, addr: SocketAddr) -> io::Result<BoxedUdp> {
        let proxy_stream = BufWriter::new(TcpStream::connect(&self.server).await?);
        let socket = UdpSocket::bind(addr).await?;
        let udp =
            SocksDatagram::associate(proxy_stream, socket, self.auth.clone(), None::<SocketAddr>)
                .await
                .map_err(other)?;

        Ok(udp.boxed())
    }
}

#[cfg(test)]
pub mod test {
    use async_std::net::TcpListener;
    use async_std::task::JoinHandle;
    use fast_socks5::server::{Config, Socks5Socket};
    use std::io;
    use std::net::SocketAddr;

    pub async fn socks5_server() -> (JoinHandle<io::Result<()>>, SocketAddr) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let config = Config::default();
        (
            async_std::task::spawn(async move {
                let (socket, _) = listener.accept().await?;
                let socket = Socks5Socket::new(socket, std::sync::Arc::new(config));
                socket
                    .upgrade_to_socks5()
                    .await
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                Ok(())
            }),
            addr,
        )
    }
}
