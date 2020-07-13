mod protocol;

use crate::interface::{IntercepterFactory, Packet, BorrowedPacket};
use crate::rt::{Sender, Mutex, UdpSocket, interval, Duration, channel, Receiver};
use futures::stream::StreamExt;
use std::sync::Arc;
use std::io;
use protocol::{ForwarderFrame, Builder, Ipv4};
use smoltcp::wire::{EthernetFrame, Ipv4Packet, Ipv4Cidr};

#[derive(Debug)]
struct Inner {
    socket: UdpSocket,
}

#[derive(Debug, Clone)]
pub struct LanClient {
    relay_server: String,
    inner: Arc<Mutex<Inner>>,
    cidr: Ipv4Cidr,
}

struct LanClientIntercepter {
    inner: Arc<Mutex<Inner>>,
    sender: Sender<Packet>,
    cidr: Ipv4Cidr,
}

impl LanClientIntercepter {
    fn process(&self, pkt: &BorrowedPacket) -> bool {
        fn process(cidr: Ipv4Cidr, pkt: &BorrowedPacket) -> crate::Result<()> {
            let packet = EthernetFrame::new_checked(pkt as &[u8])?;
            let packet = Ipv4Packet::new_checked(packet.payload())?;
            if cidr.contains_addr(&packet.src_addr()) && cidr.contains_addr(&packet.dst_addr()) {
                let packet = ForwarderFrame::Ipv4(Ipv4::new(&pkt));
                return Err(crate::error::Error::BadPacket);
            }
            Ok(())
        }
        process(self.cidr, pkt).is_err()
    }
}

impl LanClient {
    pub async fn new(relay_server: String, cidr: Ipv4Cidr) -> io::Result<LanClient> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(&relay_server).await?;
        let inner = Arc::new(Mutex::new(Inner {
            socket,
        }));
        let inner2 = inner.clone();
        crate::rt::spawn(interval(Duration::from_secs(30))
            .for_each(move |_| {
                let inner = inner2.clone();
                async move {
                    let mut inner = inner.lock().await;
                    let keepalive = ForwarderFrame::Keepalive;
                    inner.socket.send(&keepalive.build()).await.unwrap();
                }
            })
        );
        crate::rt::spawn(async {

        });
        Ok(LanClient {
            relay_server,
            inner,
            cidr,
        })
    }
    pub fn to_intercepter_factory(&self) -> IntercepterFactory {
        let inner = self.inner.clone();
        let cidr = self.cidr.clone();
        Box::new(move |sender: Sender<Packet>| {
            let intercepter = LanClientIntercepter {
                inner: inner.clone(),
                sender,
                cidr,
            };
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
