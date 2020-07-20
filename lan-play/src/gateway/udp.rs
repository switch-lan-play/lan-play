use crate::future_smoltcp::{OwnedUdp, UdpSocket};
use crate::proxy::{other, BoxProxy, SendHalf, RecvHalf, new_udp_timeout};
use crate::rt::{Sender, channel, Mutex};
use drop_abort::{abortable, DropAbortHandle};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use futures::{select, future::FutureExt};
use lru::LruCache;

pub(super) struct UdpGateway {
    proxy: Arc<BoxProxy>,
    cache: Mutex<LruCache<SocketAddr, UdpConnection>>,
}

impl UdpGateway {
    pub fn new(proxy: Arc<BoxProxy>) -> UdpGateway {
        UdpGateway {
            proxy,
            cache: Mutex::new(LruCache::new(100))
        }
    }
    pub async fn process(&self, mut udp: UdpSocket) -> io::Result<()> {
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
    async fn on_udp(&self, udp: OwnedUdp, sender: Sender<OwnedUdp>) -> io::Result<()> {
        let mut cache = self.cache.lock().await;
        let src = udp.src();
        if !cache.contains(&src) {
            cache.put(src, UdpConnection::new(&self.proxy, sender, src).await?);
            log::trace!("new udp from {:?}", src);
        }
        let connection = cache.get_mut(&src).unwrap();
        connection.send_to(&udp.data, udp.dst()).await?;
        Ok(())
    }
}

pub(super) struct UdpConnection {
    sender: SendHalf,
    _handle: DropAbortHandle,
}

impl Drop for UdpConnection {
    fn drop(&mut self) {
        log::trace!("drop UdpConnection");
    }
}

impl UdpConnection {
    pub(super) async fn run(
        mut rx: RecvHalf,
        sender: Sender<OwnedUdp>,
        src: SocketAddr,
    ) -> io::Result<()> {
        loop {
            let mut buf = vec![0; 2048];
            let (size, addr) = rx.recv_from(&mut buf).await?;
            buf.truncate(size);
            sender
                .try_send(OwnedUdp::new(addr, src, buf))
                .map_err(other)?;
        }
    }
    pub(super) async fn new(
        proxy: &Arc<BoxProxy>,
        sender: Sender<OwnedUdp>,
        src: SocketAddr,
    ) -> io::Result<UdpConnection> {
        let proxy = proxy.clone();
        let (tx, rx) = new_udp_timeout(&proxy, "0.0.0.0:0".parse().unwrap()).await?.split();
        let (fut, _handle) = abortable(UdpConnection::run(rx, sender, src));
        crate::rt::spawn(fut);
        Ok(UdpConnection {
            sender: tx,
            _handle
        })
    }
    pub(super) async fn send_to(&mut self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        self.sender.send_to(buf, addr).await
    }
}
