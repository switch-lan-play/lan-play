mod peekable_receiver;
mod socket;
mod socketset;

use socketset::SocketSet;
use smoltcp::{
    iface::{EthernetInterfaceBuilder, NeighborCache, EthernetInterface as SmoltcpEthernetInterface},
    wire::{EthernetAddress, IpCidr},
    socket::{SocketHandle, Socket},
    time::{Instant, Duration},
    phy::{Device, DeviceCapabilities, RxToken, TxToken},
};
use std::collections::BTreeMap;
use futures::select;
use futures::prelude::*;
use tokio::time::{delay_for};
use tokio::sync::{mpsc, oneshot};
use peekable_receiver::PeekableReceiver;
use crate::rawsock_socket::RawsockInterface;

#[derive(Debug)]
enum Event {
    // NewSocket(Socket<'static, 'static>, oneshot::Sender<SocketHandle>),
    // RemoveSocket(SocketHandle),
    // SocketSet(Box<dyn FnOnce(&mut SocketSet)>)
}

type Packet = Vec<u8>;

struct EthernetRunner {
    inner: SmoltcpEthernetInterface<'static, 'static, 'static, FutureDevice>,
    sockets: SocketSet,
    event_recv: mpsc::Receiver<Event>,
}

pub struct EthernetInterface {
    event_send: mpsc::Sender<Event>,
}

impl EthernetInterface {
    pub fn new(ethernet_addr: EthernetAddress, ip_addrs: Vec<IpCidr>, interf: RawsockInterface) -> EthernetInterface {
        let (event_send, event_recv) = mpsc::channel(1);
        let (tx, rx) = interf.into_streams();
        let device = FutureDevice::new(tx, rx);
        let neighbor_cache = NeighborCache::new(BTreeMap::new());
        let inner = EthernetInterfaceBuilder::new(device)
            .ethernet_addr(ethernet_addr)
            .ip_addrs(ip_addrs)
            .neighbor_cache(neighbor_cache)
            .finalize();
        let sockets = SocketSet::new();

        tokio::spawn(Self::run(EthernetRunner {
            inner,
            sockets,
            event_recv,
        }));
        
        EthernetInterface {
            event_send,
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
    pub fn push_socket(&self, handle: SocketHandle) {
        
    }
    async fn run(mut args: EthernetRunner) {
        let default_timeout = Duration::from_millis(1000);
        let EthernetRunner { inner, sockets, event_recv } = &mut args;

        loop {
            let start = Instant::now();
            let deadline = inner.poll_delay(sockets.as_set_mut(), start).unwrap_or(default_timeout);
            let device = inner.device_mut();

            select! {
                _ = delay_for(deadline.into()).fuse() => (),
                _ = device.receiver.peek().fuse() => (),
                // e = event_recv.recv().fuse() => {
                //     let e = match e {
                //         Some(e) => e,
                //         None => return,
                //     };
                //     match e {
                //         Event::SocketSet(func) => {
                //             func(sockets)
                //         }
                //     }
                // },
                default => (),
            }
            let end = Instant::now();
            let readiness = match inner.poll(sockets.as_set_mut(), end) {
                Ok(b) => b,
                Err(e) => {
                    log::error!("poll error {:?}", e);
                    return;
                },
            };

            if readiness {

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
        FutureDevice {
            caps: DeviceCapabilities::default(),
            receiver: PeekableReceiver::new(rx),
            sender: tx,
        }
    }
    fn new2() -> (FutureDevice, (mpsc::Sender<Packet>, mpsc::Receiver<Packet>)) {
        let (recv_tx, recv_rx) = mpsc::channel(1);
        let (send_tx, send_rx) = mpsc::channel(1);
        (FutureDevice {
            caps: DeviceCapabilities::default(),
            receiver: PeekableReceiver::new(recv_rx),
            sender: send_tx,
        }, (recv_tx, send_rx))
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