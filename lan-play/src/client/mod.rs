mod protocol;

use crate::interface::{IntercepterFactory, Packet, BorrowedPacket};
use tokio::sync::{mpsc::UnboundedSender, Mutex};
use tokio::net::UdpSocket;
use tokio::time::{interval, Duration};
use futures::stream::StreamExt;
use std::sync::Arc;
use std::io;
use protocol::{ForwarderFrame, Builder, Ipv4};

#[derive(Debug)]
struct Inner {
    socket: UdpSocket,
}

#[derive(Debug, Clone)]
pub struct LanClient {
    relay_server: String,
    inner: Arc<Mutex<Inner>>,
}

struct LanClientIntercepter {
    inner: Arc<Mutex<Inner>>,
    sender: UnboundedSender<Packet>,
}

impl LanClientIntercepter {
    fn process(&self, pkt: &BorrowedPacket) -> bool {
        let pkt = pkt.to_owned().to_vec();
        let packet = ForwarderFrame::Ipv4(Ipv4::new(&pkt));
        // self.sender.send(packet.build()).expect("Failed to send from LanClientIntercepter");
        false
    }
}

impl LanClient {
    pub async fn new(relay_server: String) -> io::Result<LanClient> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(&relay_server).await?;
        let inner = Arc::new(Mutex::new(Inner {
            socket,
        }));
        let inner2 = inner.clone();
        tokio::spawn(interval(Duration::from_secs(30))
            .for_each(move |_| {
                let inner = inner2.clone();
                async move {
                    let mut inner = inner.lock().await;
                    let keepalive = ForwarderFrame::Keepalive;
                    inner.socket.send(&keepalive.build()).await.unwrap();
                }
            })
        );
        Ok(LanClient {
            relay_server,
            inner,
        })
    }
    pub fn to_intercepter_factory(&self) -> IntercepterFactory {
        let inner = self.inner.clone();
        Box::new(move |sender: UnboundedSender<Packet>| {
            let intercepter = LanClientIntercepter { inner: inner.clone(), sender };
            Box::new(move |pkt: &BorrowedPacket| {
                intercepter.process(pkt)
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
