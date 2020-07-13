mod protocol;

use crate::interface::{IntercepterFactory, IntercepterFn, Packet, BorrowedPacket};
use crate::rt::{Sender, Mutex, UdpSocket, interval, Duration, channel, Receiver};
use futures::stream::StreamExt;
use std::sync::{Arc, Mutex as SyncMutex};
use std::{collections::HashMap, io};
use protocol::{ForwarderFrame, Builder, Ipv4};
use smoltcp::wire::{EthernetFrame, Ipv4Address, Ipv4Packet, Ipv4Cidr};
use futures::{select, prelude::*};

#[derive(Debug)]
struct Inner {
    socket: Mutex<UdpSocket>,
    map_sender: SyncMutex<HashMap<Ipv4Address, Sender<Packet>>>,
    all_sender: SyncMutex<Vec<Sender<Packet>>>,
    // send by udp to server
    tx: Sender<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct LanClient {
    relay_server: String,
    inner: Arc<Inner>,
    cidr: Ipv4Cidr,
}

struct LanClientIntercepter {
    inner: Arc<Inner>,
    cidr: Ipv4Cidr,
}

impl LanClientIntercepter {
    fn new(inner: Arc<Inner>, cidr: Ipv4Cidr) -> IntercepterFactory {
        Box::new(move |sender: Sender<Packet>| {
            let intercepter = LanClientIntercepter {
                inner: inner.clone(),
                cidr,
            };
            inner.all_sender.lock().unwrap().push(sender.clone());
            let tx = inner.tx.clone();
            Box::new(move |pkt: &BorrowedPacket| {
                intercepter.process(pkt, tx.clone())
            })
        })
    }
    fn process(&self, pkt: &BorrowedPacket, tx: Sender<Vec<u8>>) -> bool {
        fn process(cidr: Ipv4Cidr, pkt: &BorrowedPacket, tx: Sender<Vec<u8>>) -> crate::Result<()> {
            let packet = EthernetFrame::new_checked(pkt as &[u8])?;
            let packet = Ipv4Packet::new_checked(packet.payload())?;
            if cidr.contains_addr(&packet.src_addr()) && cidr.contains_addr(&packet.dst_addr()) {
                let packet = ForwarderFrame::Ipv4(Ipv4::new(&pkt));
                let packet = packet.build();
                tx.try_send(packet).unwrap();
                return Err(crate::error::Error::BadPacket);
            }
            Ok(())
        }
        process(self.cidr, pkt, tx).is_err()
    }
}

impl LanClient {
    async fn process(inner: Arc<Inner>, rx: Receiver<Vec<u8>>) {
        let mut interval = interval(Duration::from_secs(30));
        let mut socket = inner.socket.lock().await;
        loop {
            let mut buf = [0u8; 2048];
            select! {
                _ = interval.next().fuse() => {
                    let keepalive = ForwarderFrame::Keepalive;
                    socket.send(&keepalive.build()).await.unwrap();
                }
                pkt = rx.recv().fuse() => {
                    match pkt {
                        Ok(pkt) => {
                            socket.send(&pkt).await.unwrap();
                        }
                        Err(e) => {
                            log::error!("lan client process err {:?}", e);
                            break
                        }
                    }
                }
                a = socket.recv(&mut buf).fuse() => {
                    
                }
            }
        }
    }
    pub async fn new(relay_server: String, cidr: Ipv4Cidr) -> io::Result<LanClient> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(&relay_server).await?;
        let (tx, rx) = channel();
        let inner = Arc::new(Inner {
            socket: Mutex::new(socket),
            map_sender: SyncMutex::new(HashMap::new()),
            all_sender: SyncMutex::new(Vec::new()),
            tx,
        });
        crate::rt::spawn(Self::process(inner.clone(), rx));
        Ok(LanClient {
            relay_server,
            inner,
            cidr,
        })
    }
    pub fn to_intercepter_factory(&self) -> IntercepterFactory {
        let inner = self.inner.clone();
        let cidr = self.cidr.clone();
        LanClientIntercepter::new(inner, cidr)
    }
    pub async fn ping(&self) -> io::Result<()> {
        let mut socket = self.inner.socket.lock().await;
        let content = b"\x021234";

        socket.send(content).await?;
        let mut buf = vec![0u8; 5];
        let size = socket.recv(&mut buf).await?;
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
