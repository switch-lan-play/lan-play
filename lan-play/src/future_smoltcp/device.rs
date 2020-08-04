use async_channel::{Sender, Receiver, TryRecvError};
use smoltcp::{
    phy::{DeviceCapabilities, RxToken, TxToken},
    time::Instant,
};
use std::task::Waker;
use std::collections::VecDeque;
use futures::{Stream, Sink};
use futures::prelude::*;
use super::channel_sink::SinkSender;

pub type Packet = Vec<u8>;

pub type ChannelDevice = FutureDevice<Receiver<Packet>, SinkSender<Packet>>;

pub struct FutureDevice<In, Out>
where
    In: Stream<Item = Packet>,
    Out: Sink<Packet>,
{
    caps: DeviceCapabilities,
    receiver: In,
    sender: Out,
    temp: Option<Packet>,
    pub waker: Option<Waker>,

    buf: VecDeque<Packet>,
}

impl<In, Out> FutureDevice<In, Out>
where
    In: Stream<Item = Packet> + Unpin,
    Out: Sink<Packet>,
{
    pub fn new(tx: Out, rx: In, mtu: usize) -> FutureDevice<In, Out> {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = mtu;
        caps.max_burst_size = Some(1);
        FutureDevice {
            caps,
            receiver: rx,
            sender: tx,
            temp: None,
            waker: None,
            buf: VecDeque::with_capacity(100),
        }
    }
    pub fn need_wait(&self) -> bool {
        self.temp.is_none()
    }
    pub async fn wait(&mut self) {
        self.temp = self.receiver.next().await;
    }
    fn get_next(&mut self) -> Option<Packet> {
        if let Some(t) = self.temp.take() {
            return Some(t);
        }
        unimplemented!();
        // match self.receiver.try_recv() {
        //     Ok(p) => Some(p),
        //     Err(TryRecvError::Empty) => None,
        //     Err(TryRecvError::Closed) => todo!("handle receiver closed"),
        // }
    }
}

pub struct FutureRxToken(Packet);

impl RxToken for FutureRxToken {
    fn consume<R, F>(mut self, _timestamp: Instant, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        let p = &mut self.0;
        let result = f(p);
        result
    }
}

pub struct FutureTxToken(Sender<Packet>);

impl TxToken for FutureTxToken {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        let mut buffer = vec![0u8; len];
        let result = f(&mut buffer);
        if result.is_ok() {
            let s = self.0;
            if let Err(_e) = s.try_send(buffer) {
                log::warn!("send error");
            }
        }
        result
    }
}

impl<'d, In, Out> smoltcp::phy::Device<'d> for FutureDevice<In, Out>
where
    In: Stream<Item = Packet> + Unpin,
    Out: Sink<Packet>,
{
    type RxToken = FutureRxToken;
    type TxToken = FutureTxToken;

    fn receive(&'d mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        unimplemented!();
        // self.get_next().map(|p| (FutureRxToken(p), FutureTxToken(self.sender.clone())))
    }
    fn transmit(&'d mut self) -> Option<Self::TxToken> {
        unimplemented!();
        // Some(FutureTxToken(self.sender.clone()))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        self.caps.clone()
    }
}
