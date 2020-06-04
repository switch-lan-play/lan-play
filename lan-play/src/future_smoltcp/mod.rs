mod peekable_receiver;
mod socket;
mod socketset;

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
use tokio::time::{delay_for};
use tokio::sync::mpsc;
use peekable_receiver::PeekableReceiver;
use crate::rawsock_socket::RawsockInterface;
use socket::TcpSocket;

type Packet = Vec<u8>;

struct EthernetRunner {
    inner: SmoltcpEthernetInterface<'static, 'static, 'static, FutureDevice>,
    socket_sender: mpsc::Sender<TcpSocket>,
}

pub struct EthernetInterface {
    socket_stream: mpsc::Receiver<TcpSocket>,
}

// impl Stream for EthernetInterface {
//     type Item = ESocket;
//     fn poll_next(
//         self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//     ) -> Poll<Option<Self::Item>> {

//     }
// }

impl EthernetInterface {
    pub fn new(ethernet_addr: EthernetAddress, ip_addrs: Vec<IpCidr>, gateway_ip: Ipv4Address, interf: RawsockInterface) -> EthernetInterface {
        let (socket_send, socket_recv) = mpsc::channel(1);
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

        tokio::spawn(Self::run(EthernetRunner {
            inner,
            socket_sender: socket_send,
        }));
        
        EthernetInterface {
            socket_stream: socket_recv,
        }
    }
    // async fn new_socket<T>(&self, socket: T) -> SocketHandle
    // where
    //     T: Into<Socket<'static, 'static>>,
    // {
    //     let (tx, rx) = oneshot::channel();
    //     self.event_send.clone().send(Event::NewSocket(socket.into(), tx)).await;
    //     rx.await.unwrap()
    // }
    // fn remove_socket(&self, handle: SocketHandle) {
    //     self.event_send.clone().try_send(Event::RemoveSocket(handle)).unwrap()
    // }
    pub async fn next_socket(&mut self) -> Option<TcpSocket> {
        self.socket_stream.recv().await
    }
    async fn run(args: EthernetRunner) {
        let default_timeout = Duration::from_millis(1000);
        let EthernetRunner {
            mut inner,
            socket_sender, 
        } = args;
        let mut sockets = SocketSet::new(socket_sender);

        loop {
            let start = Instant::now();
            let deadline = inner.poll_delay(sockets.as_set_mut(), start).unwrap_or(default_timeout);
            let device = inner.device_mut();

            select! {
                _ = delay_for(deadline.into()).fuse() => {},
                _ = device.receiver.peek().fuse() => {},
            }
            let end = Instant::now();
            let readiness = match inner.poll(sockets.as_set_mut(), end) {
                Ok(b) => b,
                Err(e) => {
                    log::error!("poll error {:?}", e);
                    return;
                },
            };

            if !readiness { continue }
            sockets.process().await;
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
            // TODO handle receiver closed
            _ => None
        }
    }
    fn transmit(&'d mut self) -> Option<Self::TxToken> {
        Some(FutureTxToken(self.sender.clone()))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        self.caps.clone()
    }
}