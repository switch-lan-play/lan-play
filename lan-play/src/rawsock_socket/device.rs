use smoltcp::phy::{DeviceCapabilities,RxToken,TxToken};
use smoltcp::{
    iface::{EthernetInterfaceBuilder, NeighborCache, EthernetInterface},
    wire::{IpCidr, EthernetAddress},
    socket::{SocketSet},
    time::{Instant},
};
use crate::channel_port::{ChannelPort, Sender};

pub type Packet = Vec<u8>;
pub struct ChannelDevice {
    port: ChannelPort<Packet>,
}

impl ChannelDevice {
    pub fn new(port: ChannelPort<Packet>) -> ChannelDevice {
        ChannelDevice {
            port
        }
    }
}

pub struct RawRxToken(Packet);

impl RxToken for RawRxToken {
    fn consume<R, F>(mut self, _timestamp: Instant, f: F) -> smoltcp::Result<R>
        where F: FnOnce(&mut [u8]) -> smoltcp::Result<R>
    {
        let p = &mut self.0;
        let result = f(p);
        result
    }
}


pub struct RawTxToken(Sender::<Packet>);

impl<'a> TxToken for RawTxToken {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> smoltcp::Result<R>
        where F: FnOnce(&mut [u8]) -> smoltcp::Result<R>
    {
        let mut buffer = Vec::new();
        buffer.resize(len, 0);
        let result = f(&mut buffer);
        let sender = self.0;
        let sent = sender.send(buffer);
        if !sent.is_ok() {
            println!("send failed {}", len);
        }
        result
    }
}

impl<'d> smoltcp::phy::Device<'d> for ChannelDevice {
    type RxToken = RawRxToken;
    type TxToken = RawTxToken;

    fn receive(&'d mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        self.port.try_recv().ok().map(|packet| {(
            RawRxToken(packet),
            RawTxToken(self.port.clone_sender())
        )})
    }
    fn transmit(&'d mut self) -> Option<Self::TxToken> {
        Some(RawTxToken(self.port.clone_sender()))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1536;
        caps.max_burst_size = Some(1);
        caps
    }
}
