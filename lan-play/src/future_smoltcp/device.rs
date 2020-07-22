
use super::peekable_receiver::PeekableReceiver;

use async_channel::{Sender, Receiver, TryRecvError};
use smoltcp::{
    phy::{DeviceCapabilities, RxToken, TxToken},
    time::Instant,
};

type Packet = Vec<u8>;

pub struct FutureDevice {
    caps: DeviceCapabilities,
    receiver: PeekableReceiver<Packet>,
    sender: Sender<Packet>,
}

impl FutureDevice {
    pub fn new(tx: Sender<Packet>, rx: Receiver<Packet>) -> FutureDevice {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1536;
        caps.max_burst_size = Some(1);
        FutureDevice {
            caps,
            receiver: PeekableReceiver::new(rx),
            sender: tx,
        }
    }
    pub async fn wait(&mut self) {
        self.receiver.peek().await;
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
        match self.receiver.try_recv() {
            Ok(packet) => Some((FutureRxToken(packet), FutureTxToken(self.sender.clone()))),
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
