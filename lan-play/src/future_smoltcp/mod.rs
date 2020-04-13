use smoltcp::{
    iface::{EthernetInterfaceBuilder, NeighborCache, EthernetInterface as SmoltcpEthernetInterface},
    wire::{EthernetAddress, IpCidr},
    socket::{SocketSet, SocketHandle, AnySocket},
    time::{Instant},
    phy::{Device, DeviceCapabilities, RxToken, TxToken},
};
use std::collections::BTreeMap;
use futures::stream::Stream;
use futures::sink::{Sink, SinkExt};
use futures::executor::block_on;
use tokio::sync::mpsc;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

type Packet = Vec<u8>;

pub struct EthernetInterface {
    inner: SmoltcpEthernetInterface<'static, 'static, 'static, FutureDevice>,
    receiver: mpsc::Receiver<Packet>,
    sender: mpsc::Sender<Packet>,
    sockets: SocketSet<'static, 'static, 'static>,
}

impl EthernetInterface {
    pub fn new(ethernet_addr: EthernetAddress, ip_addrs: Vec<IpCidr>) -> EthernetInterface {
        let (device, (sender, receiver)) = FutureDevice::new2();
        let neighbor_cache = NeighborCache::new(BTreeMap::new());
        let inner = EthernetInterfaceBuilder::new(device)
            .ethernet_addr(ethernet_addr)
            .ip_addrs(ip_addrs)
            .neighbor_cache(neighbor_cache)
            .finalize();
        let sockets = SocketSet::new(vec![]);

        EthernetInterface {
            inner,
            receiver,
            sender,
            sockets,
        }
    }
}

impl Stream for EthernetInterface {
    type Item = Packet;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let device = self.inner.device_mut();
        device.set_waker(cx.waker().clone());
        match self.inner.poll(&mut self.sockets, Instant::now()) {

        }

    }
}

pub struct FutureDevice {
    caps: DeviceCapabilities,
    receiver: mpsc::Receiver<Packet>,
    sender: mpsc::Sender<Packet>,
    waker: Option<Waker>,
}

impl FutureDevice {
    fn new2() -> (FutureDevice, (mpsc::Sender<Packet>, mpsc::Receiver<Packet>)) {
        let (recv_tx, recv_rx) = mpsc::channel(1);
        let (send_tx, send_rx) = mpsc::channel(1);
        (FutureDevice {
            caps: DeviceCapabilities::default(),
            receiver: recv_rx,
            sender: send_tx,
            waker: None,
        }, (recv_tx, send_rx))
    }
    fn set_waker(&mut self, waker: Waker) {
        self.waker = Some(waker)
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