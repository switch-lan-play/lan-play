use super::{other, socket, BoxProxy, BoxTcp, BoxUdp, Proxy, SocketAddr, Auth};
use async_socks5::{connect, AddrKind, SocksDatagram};
use tokio::io;
use tokio::net::{TcpStream, UdpSocket};


#[async_trait]
impl socket::Udp for SocksDatagram<TcpStream> {
    async fn send_to(&mut self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        SocksDatagram::send_to(self, buf, addr).await.map_err(other)
    }
    async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        SocksDatagram::recv_from(self, buf)
            .await
            .map_err(other)
            .and_then(|(size, addr)| match addr {
                AddrKind::Ip(addr) => Ok((size, addr)),
                AddrKind::Domain(domain, port) => {
                    let err = format!("Socks5 udp recv_from get domain {}:{}", domain, port);
                    Err(other(err))
                }
            })
    }
}

pub struct Socks5Proxy {
    server: String,
    auth: Option<async_socks5::Auth>,
}

impl Socks5Proxy {
    pub fn new(server: String, auth: Option<Auth>) -> BoxProxy {
        Box::new(Self {
            server,
            auth: auth.map(|a| async_socks5::Auth::new(a.username, a.password))
        })
    }
}

#[async_trait]
impl Proxy for Socks5Proxy {
    async fn new_tcp(&self, addr: SocketAddr) -> io::Result<BoxTcp> {
        let mut socket = TcpStream::connect(&self.server).await?;
        connect(&mut socket, addr, self.auth.clone())
            .await
            .map_err(other)?;
        Ok(Box::new(socket))
    }
    async fn new_udp(&self, addr: SocketAddr) -> io::Result<BoxUdp> {
        let proxy_stream = TcpStream::connect(&self.server).await?;
        let socket = UdpSocket::bind(addr).await?;
        let udp =
            SocksDatagram::associate(proxy_stream, socket, self.auth.clone(), None::<SocketAddr>)
                .await
                .map_err(other)?;

        Ok(Box::new(udp))
    }
}

#[cfg(test)]
pub mod test {
    use async_std::net::TcpListener;
    use async_std::task::JoinHandle;
    use fast_socks5::server::{Config, Socks5Socket};
    use std::io;

    pub async fn socks5_server() -> (JoinHandle<io::Result<()>>, u16) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
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
            port,
        )
    }
}
