mod peekable_receiver;
mod socket;
mod socketset;
mod raw_udp;
mod reactor;

use socketset::SocketSet;
use smoltcp::{
    iface::{EthernetInterfaceBuilder, NeighborCache, EthernetInterface as SmoltcpEthernetInterface, Routes},
    wire::{EthernetAddress, IpCidr, Ipv4Address},
    time::Instant,
    phy::{DeviceCapabilities, RxToken, TxToken},
};
use std::collections::BTreeMap;
use tokio::sync::mpsc::{self, error::TryRecvError};
use peekable_receiver::PeekableReceiver;
use crate::rawsock_socket::RawsockInterface;
pub use socket::{Socket, TcpListener, TcpSocket, UdpSocket, SocketHandle};
use std::sync::{Arc, Mutex};
use reactor::{NetReactor, ReactorRunner};

type Packet = Vec<u8>;
pub type OutPacket = (SocketHandle, Packet);
pub type Ethernet = SmoltcpEthernetInterface<'static, 'static, 'static, FutureDevice>;

pub struct Net {
    socket_stream: mpsc::Receiver<Socket>,
    reactor: NetReactor,
    listener: TcpListener,
}

impl Net {
    pub fn new(ethernet_addr: EthernetAddress, ip_addrs: Vec<IpCidr>, gateway_ip: Ipv4Address, interf: RawsockInterface) -> Net {
        let (socket_send, socket_recv) = mpsc::channel(1);
        let (packet_sender, packet_receiver) = mpsc::channel(1);

        let (_running, tx, rx) = interf.start();
        let device = FutureDevice::new(tx, rx);
        let neighbor_cache = NeighborCache::new(BTreeMap::new());
        let mut routes = Routes::new(BTreeMap::new());
        routes.add_default_ipv4_route(gateway_ip).unwrap();

        let ethernet = EthernetInterfaceBuilder::new(device)
            .ethernet_addr(ethernet_addr)
            .ip_addrs(ip_addrs)
            .neighbor_cache(neighbor_cache)
            .any_ip(true)
            .routes(routes)
            .finalize();

        let socket_set = Arc::new(Mutex::new(
            SocketSet::new(socket_send, packet_sender)
        ));
        let reactor = NetReactor::new(socket_set);
        let r = reactor.clone();
        tokio::spawn(async move {
            r.run(ReactorRunner {
                ethernet,
                packet_receiver,
            }).await
        });
        
        Net {
            socket_stream: socket_recv,
            listener: TcpListener::new(reactor.clone()),
            reactor,
        }
    }
    pub async fn next_socket(&mut self) -> Option<Socket> {
        self.socket_stream.recv().await
    }
}

pub struct FutureDevice {
    caps: DeviceCapabilities,
    receiver: PeekableReceiver<Packet>,
    sender: mpsc::Sender<Packet>,
}

impl FutureDevice {
    fn new(tx: mpsc::Sender<Packet>, rx: mpsc::Receiver<Packet>) -> FutureDevice {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1536;
        caps.max_burst_size = Some(1);
        FutureDevice {
            caps,
            receiver: PeekableReceiver::new(rx),
            sender: tx,
        }
    }
}

pub struct FutureRxToken(Packet);

impl RxToken for FutureRxToken {
    fn consume<R, F>(mut self, _timestamp: Instant, f: F) -> smoltcp::Result<R>
        where F: FnOnce(&mut [u8]) -> smoltcp::Result<R>
    {
        let p = &mut self.0;
        let result = f(p);
        result
    }
}


pub struct FutureTxToken(mpsc::Sender<Packet>);

impl TxToken for FutureTxToken {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> smoltcp::Result<R>
        where F: FnOnce(&mut [u8]) -> smoltcp::Result<R>
    {
        let mut buffer = vec![0u8; len];
        let result = f(&mut buffer);
        if result.is_ok() {
            let mut s = self.0;
            if s.try_send(buffer).is_err() {
                log::warn!("send error");
            }
        }
        result
    }
}

impl<'d> smoltcp::phy::Device<'d> for FutureDevice
where
{
    type RxToken = FutureRxToken;
    type TxToken = FutureTxToken;

    fn receive(&'d mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        match self.receiver.try_recv() {
            Ok(packet) => Some(
                (FutureRxToken(packet), FutureTxToken(self.sender.clone()))
            ),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Closed) => todo!("handle receiver closed"),
        }
    }
    fn transmit(&'d mut self) -> Option<Self::TxToken> {
        Some(FutureTxToken(self.sender.clone()))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        self.caps.clone()
    }
}