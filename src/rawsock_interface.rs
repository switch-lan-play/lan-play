extern crate smoltcp;
extern crate rawsock;
use smoltcp::phy::{Device,DeviceCapabilities,RxToken,TxToken};
use smoltcp::time::Instant;
use std::sync::mpsc;
use std::thread;
use std::thread::{JoinHandle};
use rawsock::traits::{Interface};
use rawsock::InterfaceDescription;

#[derive(Debug)]
pub enum Error {
    RawsockErr(rawsock::Error),
    WrongDataLink(rawsock::DataLink)
}

pub struct RawsockInterface<'a> {
    rx_buffer: [u8; 1536],
    tx_buffer: [u8; 1536],
    thread: JoinHandle<()>,
    pub interface: Box<dyn Interface<'a> + 'a>,
    pub desc: InterfaceDescription,
    stopper: std::sync::mpsc::Sender<()>
}

pub struct RawRxToken<'a>(&'a mut [u8]);

pub trait CreateDevice<'a> {
    fn create_device(&'a self, desc: InterfaceDescription) -> Result<RawsockInterface<'a>, Error>;
}

impl<'a> CreateDevice<'a> for (dyn rawsock::traits::Library + 'a) {
    fn create_device(&'a self, desc: InterfaceDescription) -> Result<RawsockInterface<'a>, Error> {
        let interface = self.open_interface(&desc.name);
        match interface {
            Err(err) => Err(Error::RawsockErr(err)),
            Ok(interface) => {
                let data_link = interface.data_link();
                if let rawsock::DataLink::Ethernet = data_link {} else {
                    return Err(Error::WrongDataLink(data_link));
                }
                let (tx, _rx) = mpsc::channel::<()>();
                let thread = thread::spawn(move || {
                    
                });
                Ok(RawsockInterface {
                    rx_buffer: [0; 1536],
                    tx_buffer: [0; 1536],
                    thread,
                    interface,
                    desc,
                    stopper: tx
                })
            }
        }
    }
}

impl<'a> RawsockInterface<'a> {
    pub fn name(&self) -> &String {
        &self.desc.name
    }
}

impl<'a> Drop for RawsockInterface<'a> {
    fn drop(&mut self) {
        // self.stopper.send(()).unwrap();
        // self.thread.join();
    }
}

impl<'a> RxToken for RawRxToken<'a> {
    fn consume<R, F>(mut self, _timestamp: Instant, f: F) -> smoltcp::Result<R>
        where F: FnOnce(&[u8]) -> smoltcp::Result<R>
    {
        // TODO: receive packet into buffer
        let result = f(&mut self.0);
        println!("rx called");
        result
    }
}


pub struct RawTxToken<'a>(&'a mut [u8]);

impl<'a> TxToken for RawTxToken<'a> {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> smoltcp::Result<R>
        where F: FnOnce(&mut [u8]) -> smoltcp::Result<R>
    {
        let result = f(&mut self.0[..len]);
        println!("tx called {}", len);
        // TODO: send packet out
        result
    }
}

impl<'a> Device<'a> for RawsockInterface<'a> {
    type RxToken = RawRxToken<'a>;
    type TxToken = RawTxToken<'a>;

    fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        Some((RawRxToken(&mut self.rx_buffer[..]),
              RawTxToken(&mut self.tx_buffer[..])))
    }

    fn transmit(&'a mut self) -> Option<Self::TxToken> {
        Some(RawTxToken(&mut self.tx_buffer[..]))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1536;
        caps.max_burst_size = Some(1);
        caps
    }
}
