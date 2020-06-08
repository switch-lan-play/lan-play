mod peekable_receiver;
mod socket;
mod socketset;
mod raw_udp;

use socketset::SocketSet;
use smoltcp::{
    iface::{EthernetInterfaceBuilder, NeighborCache, EthernetInterface as SmoltcpEthernetInterface, Routes},
    wire::{EthernetAddress, IpCidr, Ipv4Address},
    time::{Instant, Duration},
    phy::{DeviceCapabilities, RxToken, TxToken},
};
use std::collections::BTreeMap;
use futures::select;
use futures::prelude::*;
use tokio::time::delay_for;
use tokio::sync::mpsc::{self, error::TryRecvError};
use peekable_receiver::PeekableReceiver;
use crate::rawsock_socket::RawsockInterface;
pub use socket::{Socket, TcpSocket, UdpSocket, SocketHandle};
use std::sync::{Arc, Mutex};

type Packet = Vec<u8>;
pub type OutPacket = (SocketHandle, Packet);
type Ethernet = SmoltcpEthernetInterface<'static, 'static, 'static, FutureDevice>;

struct EthernetRunner {
    inner: Ethernet,
    socket_set: Arc<Mutex<SocketSet>>,
    packet_receiver: mpsc::Receiver<OutPacket>,
}

pub struct Net {
    socket_stream: mpsc::Receiver<Socket>,
    socket_set: Arc<Mutex<SocketSet>>,
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

        let inner = EthernetInterfaceBuilder::new(device)
            .ethernet_addr(ethernet_addr)
            .ip_addrs(ip_addrs)
            .neighbor_cache(neighbor_cache)
            .any_ip(true)
            .routes(routes)
            .finalize();

        let socket_set = Arc::new(Mutex::new(
            SocketSet::new(socket_send, packet_sender)
        ));
        tokio::spawn(Self::run(EthernetRunner {
            inner,
            socket_set: socket_set.clone(),
            packet_receiver,
        }));
        
        Net {
            socket_stream: socket_recv,
            socket_set,
        }
    }
    pub async fn next_socket(&mut self) -> Option<Socket> {
        self.socket_stream.recv().await
    }
    async fn run(args: EthernetRunner) {
        let default_timeout = Duration::from_millis(1000);
        let EthernetRunner {
            mut inner,
            socket_set: sockets,
            mut packet_receiver,
        } = args;

        loop {
            let start = Instant::now();
            let deadline = {
                 inner.poll_delay(
                    sockets.lock().unwrap().as_set_mut(),
                    start
                ).unwrap_or(default_timeout)
            };
            let device = inner.device_mut();

            select! {
                _ = delay_for(deadline.into()).fuse() => {},
                _ = device.receiver.peek().fuse() => {},
                item = packet_receiver.recv().fuse() => {
                    if let Some((handle, packet)) = item {
                        sockets.lock().unwrap().send(handle, packet);
                    } else {
                        break
                    }
                },
            }
            {
                let end = Instant::now();
                let mut set = sockets.lock().unwrap();
                let readiness = match inner.poll(
                    set.as_set_mut(),
                    end
                ) {
                    Ok(b) => b,
                    Err(e) => {
                        log::error!("poll error {:?}", e);
                        true
                    },
                };

                if !readiness { continue }
                set.process();
            }
        }
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