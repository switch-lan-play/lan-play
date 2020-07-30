use async_channel::{Sender, Receiver, TryRecvError};
use smoltcp::{
    phy::{DeviceCapabilities, RxToken, TxToken},
    time::Instant,
};

type Packet = Vec<u8>;

pub struct FutureDevice {
    caps: DeviceCapabilities,
    receiver: Receiver<Packet>,
    sender: Sender<Packet>,
    temp: Option<Packet>,
}

impl FutureDevice {
    pub fn new(tx: Sender<Packet>, rx: Receiver<Packet>, mtu: usize) -> FutureDevice {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = mtu;
        caps.max_burst_size = Some(1);
        FutureDevice {
            caps,
            receiver: rx,
            sender: tx,
            temp: None,
        }
    }
    pub fn need_wait(&self) -> bool {
        self.temp.is_none()
    }
    pub async fn wait(&mut self) {
        self.temp = self.receiver.recv().await.ok();
    }
    fn get_next(&mut self) -> Option<Packet> {
        if let Some(t) = self.temp.take() {
            return Some(t);
        }
        match self.receiver.try_recv() {
            Ok(p) => Some(p),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Closed) => todo!("handle receiver closed"),
        }
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

impl<'d> smoltcp::phy::Device<'d> for FutureDevice {
    type RxToken = FutureRxToken;
    type TxToken = FutureTxToken;

    fn receive(&'d mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        self.get_next().map(|p| (FutureRxToken(p), FutureTxToken(self.sender.clone())))
    }
    fn transmit(&'d mut self) -> Option<Self::TxToken> {
        Some(FutureTxToken(self.sender.clone()))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        self.caps.clone()
    }
}
