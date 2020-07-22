use crate::interface::{IntercepterFactory, Packet, BorrowedPacket};
use crate::rt::{Mutex, UdpSocket, interval, Duration};
use async_channel::{Sender, Receiver, unbounded};
use futures::stream::StreamExt;
use std::sync::{Arc, Mutex as SyncMutex};
use std::{collections::HashMap, io};
use super::protocol::{ForwarderFrame, Parser, Builder, Ipv4};
use smoltcp::wire::{EthernetFrame, Ipv4Address, Ipv4Packet, Ipv4Cidr, EthernetProtocol};
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
    sender: Sender<Packet>,
    cidr: Ipv4Cidr,
}

impl LanClientIntercepter {
    fn new(inner: Arc<Inner>, cidr: Ipv4Cidr) -> IntercepterFactory {
        Box::new(move |sender: Sender<Packet>| {
            let intercepter = LanClientIntercepter {
                inner: inner.clone(),
                sender: sender.clone(),
                cidr,
            };
            inner.all_sender.lock().unwrap().push(sender.clone());
            Box::new(move |pkt: &BorrowedPacket| {
                intercepter.process(pkt)
            })
        })
    }
    fn process(&self, pkt: &BorrowedPacket) -> bool {
        let process = || -> crate::Result<()> {
            let packet = EthernetFrame::new_checked(pkt as &[u8])?;
            if packet.ethertype() != EthernetProtocol::Ipv4 {
                return Ok(())
            }
            let packet = Ipv4Packet::new_checked(packet.payload())?;
            if self.cidr.contains_addr(&packet.src_addr()) && self.cidr.contains_addr(&packet.dst_addr()) {
                self.inner.map_sender.lock().unwrap().insert(packet.src_addr(), self.sender.clone());
                self.inner.tx.try_send(pkt.to_vec()).unwrap();
                return Err(crate::error::Error::BadPacket);
            }
            Ok(())
        };
        process().is_err()
    }
}

impl LanClient {
    async fn on_interval(socket: &mut UdpSocket) {
        let keepalive = ForwarderFrame::Keepalive;
        socket.send(&keepalive.build()).await.unwrap();
    }
    async fn on_ipv4(socket: &mut UdpSocket, pkt: &[u8]) {
        let packet = ForwarderFrame::Ipv4(Ipv4::new(pkt));
        let packet = packet.build();
        socket.send(&packet).await.unwrap();
    }
    async fn on_recv(inner: Arc<Inner>, buf: &[u8]) {
        if let Ok(p) = ForwarderFrame::parse(buf) {
            match p {
                ForwarderFrame::Ipv4(pkt) => {
                    let all_sender = inner.all_sender.lock().unwrap();
                    for i in all_sender.iter() {
                        i.try_send(pkt.payload().to_owned()).unwrap();
                    }
                }
                _ => {}
            }
        }
    }
    async fn process(inner: Arc<Inner>, rx: Receiver<Vec<u8>>) {
        let mut interval = interval(Duration::from_secs(30));
        let mut socket = inner.socket.lock().await;
        loop {
            let mut buf = [0u8; 2048];
            select! {
                _ = interval.next().fuse() => {
                    LanClient::on_interval(&mut socket).await;
                }
                pkt = rx.recv().fuse() => {
                    match pkt {
                        Ok(pkt) => {
                            LanClient::on_ipv4(&mut socket, &pkt).await;
                        }
                        Err(e) => {
                            log::error!("lan client process err {:?}", e);
                            break
                        }
                    }
                }
                r = socket.recv(&mut buf).fuse() => {
                    match r {
                        Ok(size) => {
                            let buf = &buf[..size];
                            LanClient::on_recv(inner.clone(), buf).await;
                        },
                        Err(e) => {
                            log::error!("socket recv {:?}", e);
                            break
                        }
                    }
                }
            }
        }
    }
    pub async fn new(relay_server: String, cidr: Ipv4Cidr) -> io::Result<LanClient> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(&relay_server).await?;
        let (tx, rx) = unbounded();
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
