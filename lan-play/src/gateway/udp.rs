use crate::future_smoltcp::OwnedUdp;
use crate::proxy::{other, BoxProxy, SendHalf, RecvHalf, new_udp_timeout};
use crate::rt::Sender;
use drop_abort::{abortable, DropAbortHandle};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

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
