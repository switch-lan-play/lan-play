mod timeout_stream;
mod udp;

use crate::future_smoltcp::{OwnedUdp, TcpListener, TcpSocket, UdpSocket};
use crate::proxy::{BoxProxy, new_tcp_timeout};
use crate::rt::{Mutex, copy, Sender, channel, split, Instant, prelude::*};
use lru::LruCache;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use futures::{select, try_join, future::FutureExt};
use timeout_stream::TimeoutStream;
use udp::UdpConnection;

const TCP_TIMEOUT: Duration = Duration::from_secs(60);

struct Inner {
    udp_cache: LruCache<SocketAddr, UdpConnection>,
}

pub struct Gateway {
    proxy: Arc<BoxProxy>,
    inner: Mutex<Inner>,
}

impl Gateway {
    pub fn new(proxy: BoxProxy) -> Gateway {
        Gateway {
            proxy: Arc::new(proxy),
            inner: Mutex::new(Inner {
                udp_cache: LruCache::new(100),
            }),
        }
    }
    async fn process_udp(&self, mut udp: UdpSocket) -> io::Result<()> {
        let (udp_tx, udp_rx) = channel();
        loop {
            select! {
                result = udp.recv().fuse() => {
                    let udp = result?;
                    if let Err(e) = self.on_udp(udp, udp_tx.clone()).await {
                        log::error!("on_udp {:?}", e);
                    }
                }
                p = udp_rx.recv().fuse() => {
                    if let Ok(p) = p {
                        if let Err(e) = udp.send(p).await {
                            log::error!("udp.send {:?}", e);
                        }
                    } else {
                        todo!("handle error");
                    }
                }
            }
        }
    }
    async fn process_tcp(&self, mut listener: TcpListener) -> io::Result<()> {
        loop {
            let tcp = listener.accept().await?;
            let (local_addr, peer_addr) = (tcp.local_addr(), tcp.peer_addr());
            if let Err(e) = self.on_tcp(tcp).await {
                log::error!("on_tcp {:?}", e);
            }
            log::trace!("new tcp  {:?} -> {:?}", peer_addr, local_addr);
        }
    }
    pub async fn process(&self, tcp: TcpListener, udp: UdpSocket) -> io::Result<()> {
        try_join!(self.process_tcp(tcp), self.process_udp(udp))?;
        Ok(())
    }
    async fn on_udp(&self, udp: OwnedUdp, sender: Sender<OwnedUdp>) -> io::Result<()> {
        let mut inner = self.inner.lock().await;
        let src = udp.src();
        if !inner.udp_cache.contains(&src) {
            inner
                .udp_cache
                .put(src, UdpConnection::new(&self.proxy, sender, src).await?);
            log::trace!("new udp from {:?}", src);
        }
        let connection = inner.udp_cache.get_mut(&src).unwrap();
        connection.send_to(&udp.data, udp.dst()).await?;
        Ok(())
    }
    async fn on_tcp(&self, tcp: TcpSocket) -> io::Result<()> {
        TcpConnection::new(tcp, &self.proxy).await?;
        Ok(())
    }
}

struct TcpConnection {
}

impl TcpConnection {
    async fn new(stcp: TcpSocket, proxy: &Arc<BoxProxy>) -> io::Result<()> {
        let proxy = proxy.clone();

        crate::rt::spawn(async move {
            let (local_addr, peer_addr) = (stcp.local_addr(), stcp.peer_addr());
            let ptcp = TimeoutStream::new(
                new_tcp_timeout(&proxy, stcp.local_addr()?).await?,
                TCP_TIMEOUT,
            );
            let (mut s_read, mut s_write) = split(stcp);
            let (mut p_read, mut p_write) = split(ptcp);
            let start = Instant::now();

            let r = futures::future::try_join(
                async {
                    let r = copy(&mut s_read, &mut p_write).await;
                    p_write.shutdown().await?;
                    r
                },
                async {
                    let r = copy(&mut p_read, &mut s_write).await;
                    s_write.shutdown().await?;
                    r
                },
            ).await;

            log::trace!("tcp done {:?} -> {:?} {:?} {:?}", peer_addr, local_addr, r, start.elapsed());

            Ok::<(), io::Error>(())
        });
        Ok(())
    }
}

impl Drop for TcpConnection {
    fn drop(&mut self) {
        log::trace!("drop TcpConnection");
    }
}
