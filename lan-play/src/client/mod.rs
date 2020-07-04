use crate::interface::{IntercepterFactory, Packet, BorrowedPacket};
use tokio::sync::{mpsc::UnboundedSender, Mutex};
use tokio::net::UdpSocket;
use std::sync::Arc;
use std::io;

#[derive(Debug)]
struct Inner {
    socket: UdpSocket,
}

#[derive(Debug, Clone)]
pub struct LanClient {
    relay_server: String,
    inner: Arc<Mutex<Inner>>,
}

impl LanClient {
    pub async fn new(relay_server: String) -> io::Result<LanClient> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(&relay_server).await?;
        Ok(LanClient {
            relay_server,
            inner: Arc::new(Mutex::new(Inner {
                socket,
            }))
        })
    }
    pub fn to_intercepter_factory(&self) -> IntercepterFactory {
        Box::new(|_sender: UnboundedSender<Packet>| {
            Box::new(|_pkt: &BorrowedPacket| {
                false
            })
        })
    }
    pub async fn ping(&self) -> io::Result<()> {
        let mut inner = self.inner.lock().await;
        let content = b"\x021234";

        inner.socket.send(content).await?;
        let mut buf = vec![0u8; 5];
        let size = inner.socket.recv(&mut buf).await?;
        if size == 0 {
            return Err(io::Error::new(io::ErrorKind::Other, "the server seems not working, or blocked by the firewall"));
        }
        if size != 5 {
            log::error!("ping response size: {} {:?}. this should be a bug on server", size, &buf[0..size]);
            return Err(io::Error::new(io::ErrorKind::Other, "wrong length of ping response"));
        }
        if buf != content {
            return Err(io::Error::new(io::ErrorKind::Other, "wrong ping response"));
        }
        Ok(())
    }
}
