use crate::future_smoltcp::{OwnedUdp, TcpListener, TcpSocket, UdpSocket};
use crate::proxy::{other, BoxProxy, Udp2};
use drop_abort::{abortable, DropAbortHandle};
use lru::LruCache;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::{sync::{mpsc, Mutex}, io::{copy, split}};

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
    pub async fn process(&self, mut tcp: TcpListener, mut udp: UdpSocket) -> io::Result<()> {
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
                result = tcp.accept() => {
                    if let Ok(tcp) = result {
                        if let Err(e) = self.on_tcp(tcp).await {
                            log::error!("on_tcp {:?}", e);
                        }
                    }
                }
            };
        }
    }
    pub async fn on_udp(&self, udp: OwnedUdp, sender: mpsc::Sender<OwnedUdp>) -> io::Result<()> {
        let mut inner = self.inner.lock().await;
        let src = udp.src();
        if !inner.udp_cache.contains(&src) {
            inner
                .udp_cache
                .put(src, UdpConnection::new(&self.proxy, sender, src).await?);
        }
        let connection = inner.udp_cache.get_mut(&src).unwrap();
        connection.send_to(&udp.data, udp.dst()).await?;
        Ok(())
    }
    pub async fn on_tcp(&self, tcp: TcpSocket) -> io::Result<()> {
        TcpConnection::new(tcp, &self.proxy).await?;
        Ok(())
    }
}

struct TcpConnection {
}

impl TcpConnection {
    async fn new(stcp: TcpSocket, proxy: &BoxProxy) -> io::Result<()> {
        let ptcp = proxy.new_tcp(stcp.local_addr()?).await?;
        let (mut s_read, mut s_write) = split(stcp);
        let (mut p_read, mut p_write) = split(ptcp);
        tokio::spawn(async move {
            copy(&mut s_read, &mut p_write).await
        });
        tokio::spawn(async move {
            copy(&mut p_read, &mut s_write).await
        });
        Ok(())
    }
}

struct UdpConnection {
    pudp: Arc<Udp2>,
    _handle: DropAbortHandle,
}

impl UdpConnection {
    async fn run(
        pudp: Arc<Udp2>,
        mut sender: mpsc::Sender<OwnedUdp>,
        src: SocketAddr,
    ) -> io::Result<()> {
        loop {
            let mut buf = vec![0; 2048];
            let (size, addr) = pudp.recv_from(&mut buf).await?;
            buf.truncate(size);
            sender
                .send(OwnedUdp::new(addr, src, buf))
                .await
                .map_err(other)?;
        }
    }
    async fn new(
        proxy: &BoxProxy,
        sender: mpsc::Sender<OwnedUdp>,
        src: SocketAddr,
    ) -> io::Result<UdpConnection> {
        let pudp: Arc<Udp2> = Arc::new(proxy.new_udp("0.0.0.0:0".parse().unwrap()).await?.into());
        let (fut, _handle) = abortable(UdpConnection::run(pudp.clone(), sender, src));
        tokio::spawn(fut);
        Ok(UdpConnection { pudp, _handle })
    }
    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        self.pudp.send_to(buf, addr).await
    }
}
