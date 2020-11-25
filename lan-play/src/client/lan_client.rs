use crate::interface::Packet;
use tokio::{net::UdpSocket, time::{interval, Duration}};
use async_channel::{Sender, Receiver, unbounded};
use futures::stream::StreamExt;
use std::sync::{Arc, Mutex as SyncMutex};
use std::{collections::HashMap, io};
use super::protocol::{ForwarderFrame, Parser, Builder, Ipv4};
use smoltcp::wire::{EthernetFrame, EthernetRepr, Ipv4Address, Ipv4Packet, Ipv4Cidr, EthernetProtocol, EthernetAddress};
use futures::{select, prelude::*};

#[derive(Debug)]
struct Inner {
    socket: UdpSocket,
    map_sender: SyncMutex<HashMap<Ipv4Address, Sender<Packet>>>,
    all_sender: SyncMutex<Vec<Sender<Packet>>>,
    primary_sender: SyncMutex<Option<Sender<Packet>>>,
    self_addr: SyncMutex<Option<EthernetAddress>>,
    arp: SyncMutex<HashMap<Ipv4Address, EthernetAddress>>,
    // send by udp to server
    tx: Sender<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct LanClient {
    relay_server: String,
    inner: Arc<Inner>,
    cidr: Ipv4Cidr,
}

// struct LanClientIntercepter {
//     inner: Arc<Inner>,
//     sender: Sender<Packet>,
//     cidr: Ipv4Cidr,
// }

// impl LanClientIntercepter {
//     fn new(inner: Arc<Inner>, cidr: Ipv4Cidr) -> IntercepterFactory {
//         Box::new(move |sender: Sender<Packet>| {
//             let intercepter = LanClientIntercepter {
//                 inner: inner.clone(),
//                 sender: sender.clone(),
//                 cidr,
//             };
//             inner.all_sender.lock().unwrap().push(sender.clone());
//             Box::new(move |pkt: &BorrowedPacket| {
//                 intercepter.process(pkt)
//             })
//         })
//     }
//     fn process(&self, pkt: &BorrowedPacket) -> bool {
//         let process = || -> crate::Result<()> {
//             *self.inner.primary_sender.lock().unwrap() = Some(self.sender.clone());

//             let eth_packet = EthernetFrame::new_checked(pkt as &[u8])?;
//             if eth_packet.ethertype() != EthernetProtocol::Ipv4 {
//                 return Ok(())
//             }

//             *self.inner.self_addr.lock().unwrap() = Some(eth_packet.dst_addr());

//             let packet = Ipv4Packet::new_checked(eth_packet.payload())?;
//             if self.cidr.contains_addr(&packet.src_addr()) && self.cidr.contains_addr(&packet.dst_addr()) {
//                 self.inner.arp.lock().unwrap().insert(packet.src_addr(), eth_packet.src_addr());

//                 self.inner.map_sender.lock().unwrap().insert(packet.src_addr(), self.sender.clone());
//                 self.inner.tx.try_send(pkt.to_vec()).unwrap();
//                 return Err(crate::error::Error::BadPacket);
//             }
//             Ok(())
//         };
//         process().is_err()
//     }
// }

impl LanClient {
    async fn on_interval(socket: &UdpSocket) {
        let keepalive = ForwarderFrame::Keepalive;
        socket.send(&keepalive.build()).await.unwrap();
    }
    async fn on_ipv4(socket: &UdpSocket, pkt: &[u8]) {
        let packet = ForwarderFrame::Ipv4(Ipv4::new(pkt));
        let packet = packet.build();
        socket.send(&packet).await.unwrap();
    }
    async fn on_recv(inner: Arc<Inner>, buf: &[u8]) {
        if let Ok(p) = ForwarderFrame::parse(buf) {
            match p {
                ForwarderFrame::Ipv4(pkt) => {
                    let payload = pkt.payload();
                    let ipv4 = Ipv4Packet::new_unchecked(payload);
                    let src_addr = inner.self_addr.lock().unwrap().unwrap_or(EthernetAddress::BROADCAST);
                    let dst_addr = *inner.arp.lock().unwrap().get(&ipv4.dst_addr()).unwrap_or(&EthernetAddress::BROADCAST);

                    let repr = EthernetRepr {
                        src_addr,
                        dst_addr,
                        ethertype: EthernetProtocol::Ipv4,
                    };
                    let mut buffer = vec![0u8; payload.len() + repr.buffer_len()];
                    let mut eth_packet = EthernetFrame::new_unchecked(&mut buffer);
                    repr.emit(&mut eth_packet);
                    eth_packet.payload_mut().copy_from_slice(payload);
                    let buf = eth_packet.into_inner();

                    let all_sender = inner.primary_sender.lock().unwrap();
                    for i in all_sender.iter() {
                        i.try_send(buf.to_owned()).unwrap();
                    }
                }
                _ => {}
            }
        }
    }
    async fn process(inner: Arc<Inner>, rx: Receiver<Vec<u8>>) {
        let mut interval = interval(Duration::from_secs(30));
        let socket = &inner.socket;
        loop {
            let mut buf = [0u8; 2048];
            select! {
                _ = interval.next().fuse() => {
                    LanClient::on_interval(socket).await;
                }
                pkt = rx.recv().fuse() => {
                    match pkt {
                        Ok(pkt) => {
                            LanClient::on_ipv4(socket, &pkt).await;
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
            socket,
            map_sender: SyncMutex::new(HashMap::new()),
            all_sender: SyncMutex::new(Vec::new()),
            primary_sender: SyncMutex::new(None),
            self_addr: SyncMutex::new(None),
            arp: SyncMutex::new(HashMap::new()),
            tx,
        });
        tokio::spawn(Self::process(inner.clone(), rx));
        Ok(LanClient {
            relay_server,
            inner,
            cidr,
        })
    }
    pub async fn ping(&self) -> io::Result<()> {
        let socket = &self.inner.socket;
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
