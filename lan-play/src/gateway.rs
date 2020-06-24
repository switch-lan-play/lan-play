use crate::proxy::{BoxProxy, BoxUdp, Udp2, other};
use crate::future_smoltcp::{OwnedUdp, TcpListener, UdpSocket};
use lru::LruCache;
use tokio::sync::{Mutex, mpsc};
use std::net::SocketAddr;
use std::io;
use std::sync::Arc;
use futures::future::select_all;
use drop_abort::{abortable, DropAbortHandle};

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
            inner.udp_cache.put(src, UdpConnection::new(&self.proxy, sender, src).await?);
        }
        let connection = inner.udp_cache.get_mut(&src).unwrap();
        connection.send_to(&udp.data, udp.dst()).await?;
        Ok(())
    }
}

struct UdpConnection {
    pudp: Arc<Udp2>,
    _handle: DropAbortHandle,
}

impl UdpConnection {
    async fn run(pudp: Arc<Udp2>, mut sender: mpsc::Sender<OwnedUdp>, src: SocketAddr) -> io::Result<()> {
        loop {
            let mut buf = vec![0; 2048];
            let (size, addr) = pudp.recv_from(&mut buf).await?;
            buf.truncate(size);
            sender.send(OwnedUdp::new(addr, src, buf)).await.map_err(other)?;
        }
    }
    async fn new(proxy: &BoxProxy, sender: mpsc::Sender<OwnedUdp>, src: SocketAddr) -> io::Result<UdpConnection> {
        let pudp: Arc<Udp2> = Arc::new(proxy.new_udp("0.0.0.0:0".parse().unwrap()).await?.into());
        let (fut, _handle) = abortable(UdpConnection::run(pudp.clone(), sender, src));
        tokio::spawn(fut);
        Ok(UdpConnection {
            pudp,
            _handle,
        })
    }
    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        self.pudp.send_to(buf, addr).await
    }
}
