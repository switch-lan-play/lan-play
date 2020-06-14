use crate::proxy::{BoxProxy, BoxUdp};
use crate::future_smoltcp::{OwnedUdp, TcpListener, UdpSocket};
use lru::LruCache;
use tokio::sync::{Mutex, mpsc};
use std::net::SocketAddr;
use std::io;

struct Inner {
    udp_cache: LruCache<SocketAddr, UdpConnection>,
}

pub struct Gateway {
    proxy: BoxProxy,
    inner: Mutex<Inner>,
}

impl Gateway {
    pub fn new(proxy: BoxProxy) -> Gateway {
        Gateway {
            proxy,
            inner: Mutex::new(Inner {
                udp_cache: LruCache::new(100),
            }),
        }
    }
    pub async fn process(&self, mut _tcp: TcpListener, mut udp: UdpSocket) -> io::Result<()> {
        let (udp_tx, mut udp_rx) = mpsc::channel(10);
        loop {
            tokio::select! {
                result = udp.recv() => {
                    if let Ok(udp) = result {
                        if let Err(e) = self.on_udp(udp, udp_tx.clone()).await {
                            log::error!("on_udp {:?}", e);
                        }
                    }
                }
                p = udp_rx.recv() => {
                    if let Some(p) = p {
                        if let Err(e) = udp.send(p).await {
                            log::error!("udp.send {:?}", e);
                        }
                    }
                }
            };
        }
        Ok(())
    }
    pub async fn on_udp(&self, udp: OwnedUdp, sender: mpsc::Sender<OwnedUdp>) -> io::Result<()> {
        let mut inner = self.inner.lock().await;
        let src = udp.src();
        if !inner.udp_cache.contains(&src) {
            inner.udp_cache.put(src, UdpConnection::new(&self.proxy, sender).await?);
        }
        let connection = inner.udp_cache.get_mut(&src).unwrap();
        connection.send_to(&udp.data, udp.dst()).await?;
        Ok(())
    }
}

struct UdpConnection {
    pudp: BoxUdp,
    sender: mpsc::Sender<OwnedUdp>,
}

impl UdpConnection {
    async fn new(proxy: &BoxProxy, sender: mpsc::Sender<OwnedUdp>) -> io::Result<UdpConnection> {
        let pudp = proxy.new_udp("0.0.0.0:0".parse().unwrap()).await?;
        Ok(UdpConnection {
            pudp,
            sender,
        })
    }
    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        // self.pudp.send_to(buf, addr).await
        Ok(0)
    }
}
