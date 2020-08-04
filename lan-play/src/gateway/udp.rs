use crate::future_smoltcp::{OwnedUdp, UdpSocket, SendHalf as UdpSendHalf};
use crate::proxy::{other, BoxedProxy, SendHalf, RecvHalf};
use tokio::{spawn, sync::Mutex, time::Duration};
use drop_abort::{abortable, DropAbortHandle};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use futures::future::try_join;
use lru::LruCache;
use async_timeout::{VisitorTimeout, Visitor};
use async_channel::{Sender, Receiver, unbounded};

const UDP_TIMEOUT: Duration = Duration::from_secs(60);

pub(super) struct UdpGateway {
    proxy: Arc<BoxedProxy>,
    cache: Mutex<LruCache<SocketAddr, UdpConnection>>,
}

impl UdpGateway {
    pub fn new(proxy: Arc<BoxedProxy>) -> UdpGateway {
        UdpGateway {
            proxy,
            cache: Mutex::new(LruCache::new(100)),
        }
    }
    pub async fn process(&self, udp: UdpSocket) -> io::Result<()> {
        let (pop_tx, pop_rx) = unbounded();
        try_join(
            self.process_udp(udp, pop_tx),
            self.process_pop(pop_rx),
        ).await?;
        Ok(())
    }
    async fn process_udp(&self, udp: UdpSocket, pop_tx: Sender<SocketAddr>) -> io::Result<()> {
        let (tx, mut rx) = udp.split();
        let sender = Arc::new(Mutex::new(tx));
        loop {
            let udp = rx.recv().await?;
            if let Err(e) = self.on_udp(udp, sender.clone(), pop_tx.clone()).await {
                log::error!("on_udp {:?}", e);
            }
        }
    }
    async fn process_pop(&self, pop_rx: Receiver<SocketAddr>) -> io::Result<()> {
        loop {
            if let Ok(key) = pop_rx.recv().await {
                let mut cache = self.cache.lock().await;
                cache.pop(&key);
            } else {
                todo!("handle error");
            }
        }
    }
    async fn on_udp(&self, udp: OwnedUdp, sender: Arc<Mutex<UdpSendHalf>>, pop_tx: Sender<SocketAddr>) -> io::Result<()> {
        let mut cache = self.cache.lock().await;
        let src = udp.src();
        if !cache.contains(&src) {
            let (timeout, visitor) = VisitorTimeout::new(UDP_TIMEOUT);
            let pop_tx = pop_tx.clone();
            spawn(async move {
                let _ = timeout.await;
                pop_tx.send(src).await.unwrap();
            });
            cache.put(src, UdpConnection::new(&self.proxy, sender, src, visitor).await?);
            log::trace!("new udp from {:?}", src);
        }
        let connection = cache.get_mut(&src).unwrap();
        connection.send_to(&udp.data, &udp.dst()).await?;
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
        sender: Arc<Mutex<UdpSendHalf>>,
        src: SocketAddr,
    ) -> io::Result<()> {
        loop {
            let mut buf = vec![0; 2048];
            let (size, addr) = rx.recv_from(&mut buf).await?;
            buf.truncate(size);
            let data = OwnedUdp::new(addr, src, buf);
            sender
                .lock()
                .await
                .send(&data)
                .await
                .map_err(other)?;
        }
    }
    pub(super) async fn new(
        proxy: &Arc<BoxedProxy>,
        sender: Arc<Mutex<UdpSendHalf>>,
        src: SocketAddr,
        visitor: Visitor,
    ) -> io::Result<UdpConnection> {
        let proxy = proxy.clone();
        let (tx, rx) = proxy.new_udp_timeout("0.0.0.0:0".parse().unwrap()).await?.split();
        let (fut, _handle) = abortable(
            UdpConnection::run(rx, sender, src)
        );
        tokio::spawn(fut);
        Ok(UdpConnection {
            sender: tx,
            visitor,
            _handle
        })
    }
    pub(super) async fn send_to(&mut self, buf: &[u8], addr: &SocketAddr) -> io::Result<usize> {
        self.visitor.visit();
        self.sender.send_to(buf, addr).await
    }
}
