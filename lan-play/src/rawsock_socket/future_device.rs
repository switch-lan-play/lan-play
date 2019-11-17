use smoltcp::phy::{DeviceCapabilities,RxToken,TxToken};
use smoltcp::{
    iface::{EthernetInterfaceBuilder, NeighborCache, EthernetInterface},
    wire::{IpCidr, EthernetAddress},
    socket::{SocketSet},
    time::{Instant},
};
use futures::stream::Stream;
use futures::sink::{Sink, SinkExt};
use std::sync::{Arc, Mutex};
use std::pin::Pin;
use std::future::Future;
use std::task::{Context, Poll, Waker};
use async_std::task;

pub type Packet = Vec<u8>;

pub struct NewFuture<S: Stream, O: Sink<Packet>>(Option<Inner<S, O>>);

impl<S: Stream + Unpin, O: Sink<Packet> + Unpin> Future for NewFuture<S, O> {
    type Output = FutureDevice<S, O>;
    fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let s = self.get_mut().0.take().unwrap();
        Poll::Ready(FutureDevice {
            inner: Arc::new(Mutex::new(s)),
            cap: DeviceCapabilities::default(),
            waker: cx.waker().clone()
        })
    }
}

struct Inner<S: Stream, O: Sink<Packet>> {
    stream: S,
    output: O,
}
type InnerType<S, O> = Arc<Mutex<Inner<S, O>>>;
pub struct FutureDevice<S: Stream, O: Sink<Packet>> {
    inner: InnerType<S, O>,
    cap: DeviceCapabilities,
    waker: Waker,
}

impl<S: Stream + Unpin, O: Sink<Packet> + Unpin> FutureDevice<S, O> {
    pub fn new(stream: S, output: O) -> NewFuture<S, O> {
        NewFuture(Some(Inner {
            stream,
            output,
        }))
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


pub struct FutureTxToken<S: Stream, O: Sink<Packet>>(InnerType<S, O>);

impl<'a, S: Stream, O: Sink<Packet> + Unpin> TxToken for FutureTxToken<S, O> {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> smoltcp::Result<R>
        where F: FnOnce(&mut [u8]) -> smoltcp::Result<R>
    {
        let mut buffer = Vec::new();
        buffer.resize(len, 0);
        let result = f(&mut buffer);
        if result.is_ok() {
            let mut s = self.0.lock().unwrap();
            task::block_on(async {
                s.output.send(buffer).await;
            });
        }
        result
    }
}

impl<'d, S, O> smoltcp::phy::Device<'d> for FutureDevice<S, O>
where
    S: Stream<Item=Packet> + Unpin,
    O: Sink<Packet> + Unpin,
    S: 'd,
    O: 'd,
{
    type RxToken = FutureRxToken;
    type TxToken = FutureTxToken<S, O>;

    fn receive(&'d mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        let mut s = self.inner.lock().unwrap();
        if let Poll::Ready(Some(p)) = Stream::poll_next(Pin::new(&mut s.stream), &mut Context::from_waker(&self.waker)) {
            Some((
                FutureRxToken(p),
                FutureTxToken(self.inner.clone()),
            ))
        } else {
            None
        }
    }
    fn transmit(&'d mut self) -> Option<Self::TxToken> {
        Some(FutureTxToken(self.inner.clone()))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1536;
        caps.max_burst_size = Some(1);
        caps
    }
}
