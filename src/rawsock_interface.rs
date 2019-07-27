extern crate smoltcp;
use smoltcp::phy::{Device,DeviceCapabilities,RxToken,TxToken};
use smoltcp::time::Instant;
use smoltcp::Result;

pub struct RawsockInterface {

}

struct RawRxToken<'a>(&'a mut [u8]);

impl<'a> RxToken for RawRxToken<'a> {
    fn consume<R, F>(mut self, _timestamp: Instant, f: F) -> Result<R>
        where F: FnOnce(&mut [u8]) -> Result<R>
    {
        // TODO: receive packet into buffer
        let result = f(&mut self.0);
        println!("rx called");
        result
    }
}


struct RawTxToken<'a>(&'a mut [u8]);

impl<'a> TxToken for RawTxToken<'a> {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> Result<R>
        where F: FnOnce(&mut [u8]) -> Result<R>
    {
        let result = f(&mut self.0[..len]);
        println!("tx called {}", len);
        // TODO: send packet out
        result
    }
}

impl<'a> Device<'a> for RawsockInterface {
    type RxToken = RawRxToken<'a>;
    type TxToken = RawTxToken<'a>;

    fn receive(&mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        Ok()
    }

    fn transmit(&mut self) -> Option<Self::TxToken> {

    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1536;
        caps.max_burst_size = Some(1);
        caps
    }
}
