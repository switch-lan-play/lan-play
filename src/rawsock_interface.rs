extern crate smoltcp;
extern crate rawsock;
use smoltcp::phy::{Device,DeviceCapabilities,RxToken,TxToken};
use smoltcp::time::Instant;
use smoltcp::Result;
use smoltcp::socket::{TcpSocket, TcpState};
use smoltcp::wire::{IpRepr, TcpRepr};
use std::sync::mpsc;
use std::thread;
use std::thread::{JoinHandle};
use rawsock::traits::{Interface, Library};
use rawsock::InterfaceDescription;

pub struct RawsockInterface<'a> {
    rx_buffer: [u8; 1536],
    tx_buffer: [u8; 1536],
    thread: JoinHandle<()>,
    pub interface: Box<dyn Interface<'a> + 'a>,
    pub desc: InterfaceDescription,
    stopper: std::sync::mpsc::Sender<()>
}

pub struct RawRxToken<'a>(&'a mut [u8]);

pub trait Fuck {
    fn accepts(&self, ip_repr: &IpRepr, repr: &TcpRepr) -> bool;
}

impl<'a> Fuck for TcpSocket<'a> {
    fn accepts(&self, ip_repr: &IpRepr, repr: &TcpRepr) -> bool {
        if self.state() == TcpState::Closed { return false }

        // If we're still listening for SYNs and the packet has an ACK, it cannot
        // be destined to this socket, but another one may well listen on the same
        // local endpoint.
        if self.state() == TcpState::Listen && repr.ack_number.is_some() { return false }

        // Reject packets with a wrong destination.
        if self.local_endpoint().port != repr.dst_port { return false }
        if !self.local_endpoint().addr.is_unspecified() &&
            self.local_endpoint().addr != ip_repr.dst_addr() { return false }

        // Reject packets from a source to which we aren't connected.
        if self.remote_endpoint().port != 0 &&
            self.remote_endpoint().port != repr.src_port { return false }
        if !self.remote_endpoint().addr.is_unspecified() &&
            self.remote_endpoint().addr != ip_repr.src_addr() { return false }

        true
    }
}

impl<'a> RawsockInterface<'a> {
    // pub fn new(desc: InterfaceDescription) -> RawsockInterface<'a> {
    //     let (tx, rx) = mpsc::channel::<()>();
    //     let thread = thread::spawn(move || {
            
    //     });
    //     let interface = 
    //     RawsockInterface {
    //         rx_buffer: [0; 1536],
    //         tx_buffer: [0; 1536],
    //         thread,
    //         interface: ,
    //         desc,
    //         stopper: tx
    //     }
    // }
}

impl<'a> Drop for RawsockInterface<'a> {
    fn drop(&mut self) {
        // self.stopper.send(()).unwrap();
        // self.thread.join();
    }
}

impl<'a> RxToken for RawRxToken<'a> {
    fn consume<R, F>(mut self, _timestamp: Instant, f: F) -> Result<R>
        where F: FnOnce(&[u8]) -> Result<R>
    {
        // TODO: receive packet into buffer
        let result = f(&mut self.0);
        println!("rx called");
        result
    }
}


pub struct RawTxToken<'a>(&'a mut [u8]);

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
