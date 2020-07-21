use crate::future_smoltcp::{OwnedUdp, UdpSocket};
use crate::proxy::{other, BoxProxy, SendHalf, RecvHalf, new_udp_timeout};
use crate::rt::{spawn, Sender, channel, Mutex, Duration};
use drop_abort::{abortable, DropAbortHandle};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use futures::{select, future::FutureExt};
use lru::LruCache;
use async_timeout::{VisitorTimeout, Visitor};

const UDP_TIMEOUT: Duration = Duration::from_secs(60);

pub(super) struct UdpGateway {
    proxy: Arc<BoxProxy>,
    cache: Mutex<LruCache<SocketAddr, UdpConnection>>,
}

impl UdpGateway {
    pub fn new(proxy: Arc<BoxProxy>) -> UdpGateway {
        UdpGateway {
            proxy,
            cache: Mutex::new(LruCache::new(100)),
        }
    }
    pub async fn process(&self, mut udp: UdpSocket) -> io::Result<()> {
        let (udp_tx, udp_rx) = channel();
        let (pop_tx, pop_rx) = channel();
        loop {
            select! {
                result = udp.recv().fuse() => {
                    let udp = result?;
                    if let Err(e) = self.on_udp(udp, udp_tx.clone(), pop_tx.clone()).await {
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
                key = pop_rx.recv().fuse() => {
                    if let Ok(key) = key {
                        let mut cache = self.cache.lock().await;
                        cache.pop(&key);
                    } else {
                        todo!("handle error");
                    }
                }
            }
        }
    }
    async fn on_udp(&self, udp: OwnedUdp, sender: Sender<OwnedUdp>, pop_tx: Sender<SocketAddr>) -> io::Result<()> {
        let mut cache = self.cache.lock().await;
        let src = udp.src();
        if !cache.contains(&src) {
            let (timeout, visitor) = VisitorTimeout::new(UDP_TIMEOUT);
            let pop_tx = pop_tx.clone();
            spawn(async move {
                let _ = timeout.await;
                pop_tx.send(src).await.unwrap();
                log::trace!("Udp timeout");
            });
            cache.put(src, UdpConnection::new(&self.proxy, sender, src, visitor).await?);
            log::trace!("new udp from {:?}", src);
        }
        let connection = cache.get_mut(&src).unwrap();
        connection.send_to(&udp.data, udp.dst()).await?;
        Ok(())
    }
}

pub(super) struct UdpConnection {
    sender: SendHalf,
    visitor: Visitor,
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
        visitor: Visitor,
    ) -> io::Result<UdpConnection> {
        let proxy = proxy.clone();
        let (tx, rx) = new_udp_timeout(&proxy, "0.0.0.0:0".parse().unwrap()).await?.split();
        let (fut, _handle) = abortable(
            UdpConnection::run(rx, sender, src)
        );
        crate::rt::spawn(fut);
        Ok(UdpConnection {
            sender: tx,
            visitor,
            _handle
        })
    }
    pub(super) async fn send_to(&mut self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        self.visitor.visit();
        self.sender.send_to(buf, addr).await
    }
}
