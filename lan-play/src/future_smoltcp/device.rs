use smoltcp::{
    phy::{DeviceCapabilities, RxToken, TxToken},
    time::Instant,
};
use futures::{Stream, Sink, StreamExt, stream::iter};
use std::{collections::VecDeque, io};

const MAX_QUEUE_SIZE: usize = 100;

pub trait Interface: Stream<Item=Packet> + Sink<Packet, Error=io::Error> + Unpin {
}
impl<T> Interface for T
where
    T: Stream<Item=Packet> + Sink<Packet, Error=io::Error> + Unpin,
{}
pub type Packet = Vec<u8>;

pub struct FutureDevice<S> {
    caps: DeviceCapabilities,
    stream: S,
    temp: Option<Packet>,
    send_queue: VecDeque<Packet>,
}

impl<S> FutureDevice<S>
where
    S: Interface,
{
    pub fn new(stream: S, mtu: usize) -> FutureDevice<S> {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = mtu;
        caps.max_burst_size = Some(MAX_QUEUE_SIZE);
        FutureDevice {
            caps,
            stream,
            temp: None,
            send_queue: VecDeque::with_capacity(MAX_QUEUE_SIZE),
        }
    }
    pub fn need_wait(&self) -> bool {
        self.temp.is_none()
    }
    pub async fn wait(&mut self) {
        self.temp = self.stream.next().await;
    }
    pub async fn send_queue(&mut self) -> io::Result<()> {
        let stream = iter(self.send_queue.drain(..).map(|i| Ok(i)));
        stream.forward(&mut self.stream).await?;
        Ok(())
    }
    fn get_next(&mut self) -> Option<Packet> {
        if let Some(t) = self.temp.take() {
            return Some(t);
        }
        None
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

pub struct FutureTxToken<'d, S>(&'d mut FutureDevice<S>);

impl<'d, S> TxToken for FutureTxToken<'d, S>
where
    S: Interface,
{
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        let mut buffer = vec![0u8; len];
        let result = f(&mut buffer);
        if result.is_ok() {
            self.0.send_queue.push_back(buffer);
        }
        result
    }
}

impl<'d, S> smoltcp::phy::Device<'d> for FutureDevice<S>
where
    S: Interface,
    S: 'd,
{
    type RxToken = FutureRxToken;
    type TxToken = FutureTxToken<'d, S>;

    fn receive(&'d mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        self.get_next().map(move |p| (FutureRxToken(p), FutureTxToken(self)))
    }
    fn transmit(&'d mut self) -> Option<Self::TxToken> {
        if self.send_queue.len() < MAX_QUEUE_SIZE {
            Some(FutureTxToken(self))
        } else {
            None
        }
    }
    fn capabilities(&self) -> DeviceCapabilities {
        self.caps.clone()
    }
}
