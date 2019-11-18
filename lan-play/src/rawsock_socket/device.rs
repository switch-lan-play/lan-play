use smoltcp::phy::{Device, DeviceCapabilities,RxToken,TxToken};
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
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1536;
        caps.max_burst_size = Some(1);
        Poll::Ready(FutureDevice {
            inner: Arc::new(Mutex::new(s)),
            waker: cx.waker().clone(),
            caps,
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
    caps: DeviceCapabilities,
    waker: Waker,
}

impl<S: Stream + Unpin, O: Sink<Packet> + Unpin> FutureDevice<S, O> {
    pub fn new(stream: S, output: O) -> NewFuture<S, O> {
        NewFuture(Some(Inner {
            stream,
            output,
        }))
    }
    fn set_waker(&mut self, waker: Waker) {
        self.waker = waker;
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
                if s.output.send(buffer).await.is_err() {
                    log::warn!("send error");
                }
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
        self.caps.clone()
    }
}

pub struct PollAsync<'a, 'b, 'c, 'e, S, O>
where
    S: Stream<Item=Packet> + Unpin + 'static,
    O: Sink<Packet> + Unpin + 'static,
{
    iface: &'a mut EthernetInterface<'b, 'c, 'e, FutureDevice<S, O>>,
    sockets: &'a mut SocketSet<'static, 'static, 'static>,
}

impl<'a, 'b, 'c, 'e, S, O> Future for PollAsync<'a, 'b, 'c, 'e, S, O>
where
    S: Stream<Item=Packet> + Unpin + 'static,
    O: Sink<Packet> + Unpin + 'static,
{
    type Output = smoltcp::Result<()>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        this.iface.device_mut().set_waker(cx.waker().clone());
        let timestamp = Instant::now();
        let ret = match this.iface.poll(&mut this.sockets, timestamp) {
            Ok(true) => Poll::Ready(Ok(())),
            Ok(false) => Poll::Pending,
            Err(e) => Poll::Ready(Err(e)),
        };
        ret
    }
}

pub trait AsyncIface<'b, 'c, 'e, S, O> {
    fn poll_async<'a>(&'a mut self, sockets: &'a mut SocketSet<'static, 'static, 'static>) -> PollAsync<'a, 'b, 'c, 'e, S, O>
    where
        S: Stream<Item=Packet> + Unpin + 'static,
        O: Sink<Packet> + Unpin + 'static;
}

impl<'b, 'c, 'e, S, O> AsyncIface<'b, 'c, 'e, S, O> for EthernetInterface<'b, 'c, 'e, FutureDevice<S, O>>
where
    S: Stream<Item=Packet> + Unpin + 'static,
    O: Sink<Packet> + Unpin + 'static,
{
    fn poll_async<'a>(&'a mut self, sockets: &'a mut SocketSet<'static, 'static, 'static>) -> PollAsync<'a, 'b, 'c, 'e, S, O> {
        PollAsync {
            iface: self,
            sockets,
        }
    }
}
