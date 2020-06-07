use super::{Proxy, socket, SocketAddr, BoxTcp, BoxUdp, BoxProxy};
use tokio::task::{self, JoinHandle};
use tokio::net::{TcpStream, UdpSocket};
use tokio::io;
use async_socks5::{connect, Auth, AddrKind, SocksDatagram};

fn map_socks5_err(e: async_socks5::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
}

#[async_trait(?Send)]
impl socket::Udp for SocksDatagram<TcpStream> {
    async fn send_to(&mut self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        SocksDatagram::send_to(self, buf, addr)
            .await
            .map_err(map_socks5_err)
    }
    async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        SocksDatagram::recv_from(self, buf)
            .await
            .map_err(map_socks5_err)
            .and_then(|(size, addr)| match addr {
                AddrKind::Ip(addr) => {
                    Ok((size, addr))
                },
                AddrKind::Domain(domain, port) => {
                    let err = format!("Socks5 udp recv_from get domain {}:{}", domain, port);
                    Err(io::Error::new(io::ErrorKind::Other, err))
                }
            })
    }
}

pub struct Socks5Proxy {
    server: SocketAddr,
    auth: Option<Auth>,
    join: JoinHandle<()>,
}

impl Socks5Proxy {
    pub fn new(server: SocketAddr, auth: Option<Auth>) -> BoxProxy {
        let join = task::spawn(async {

        });
        Box::new(Self {
            server,
            auth,
            join,
        })
    }
}

#[async_trait(?Send)]
impl Proxy for Socks5Proxy {
    async fn new_tcp(&mut self, addr: SocketAddr) -> io::Result<BoxTcp> {
        let mut socket = TcpStream::connect(self.server.clone()).await?;
        connect(&mut socket, addr, self.auth.clone())
            .await
            .map_err(map_socks5_err)?;
        Ok(Box::new(socket))
    }
    async fn new_udp(&mut self, addr: SocketAddr) -> io::Result<BoxUdp> {
        let proxy_stream = TcpStream::connect(self.server.clone()).await?;
        let socket = UdpSocket::bind(addr).await?;
        let udp = SocksDatagram
            ::associate(proxy_stream, socket, self.auth.clone(), None::<SocketAddr>)
            .await
            .map_err(map_socks5_err)?;

        Ok(Box::new(udp))
    }
    async fn join(&mut self) -> () {
        (&mut self.join).await.unwrap()
    }
}

#[cfg(test)]
pub mod test {
    use fast_socks5::server::{Config, Socks5Socket};
    use async_std::task::JoinHandle;
    use std::io;
    use async_std::net::TcpListener;

    pub async fn socks5_server() -> (JoinHandle<io::Result<()>>, u16) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let config = Config::default();
        (async_std::task::spawn(async move {
            let (socket, _) = listener.accept().await?;
            let socket = Socks5Socket::new(socket, std::sync::Arc::new(config));
            socket
                .upgrade_to_socks5()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Ok(())
        }), port)
    }
}
