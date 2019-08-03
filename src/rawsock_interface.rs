use crate::get_addr::{get_mac, GetAddressError};
use smoltcp::phy::{DeviceCapabilities,RxToken,TxToken};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress};
use rawsock::traits::{Interface, Library};
use rawsock::InterfaceDescription;
use crossbeam_utils::thread;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug)]
pub enum Error {
    RawsockErr(rawsock::Error),
    WrongDataLink(rawsock::DataLink),
    GetAddr(GetAddressError),
}
#[derive(Debug)]
pub struct ErrorWithDesc (pub Error, pub InterfaceDescription);

pub struct RawsockInterfaceSet {
    lib: &'static Box<dyn Library>,
    all_interf: Vec<rawsock::InterfaceDescription>,
}

pub struct RawsockInterface {
    shit: bool,
    pub desc: InterfaceDescription,
    interface: Rc<RefCell<Box<dyn Interface<'static>>>>,
    mac: EthernetAddress,
    data_link: rawsock::DataLink,
    // dummy: &'a (),
}

impl RawsockInterfaceSet {
    pub fn new(lib: &'static Box<dyn Library>) -> Result<RawsockInterfaceSet, rawsock::Error> {
        let all_interf = lib.all_interfaces()?;
        Ok(RawsockInterfaceSet {
            lib,
            all_interf,
        })
    }
    pub fn lib_version(&self) -> rawsock::LibraryVersion {
        self.lib.version()
    }
    pub fn open_all_interface(&self) -> (Vec<RawsockInterface>, Vec<ErrorWithDesc>) {
        let all_interf = self.all_interf.clone();
        let (opened, errored): (Vec<_>, _) = all_interf
            .into_iter()
            .map(|i| self.create_device(i))
            .partition(Result::is_ok);
        (
            opened.into_iter().map(Result::unwrap).collect::<Vec<_>>(),
            errored.into_iter().map(|i| i.err().unwrap()).collect::<Vec<_>>()
        )
    }
    pub fn start(&self, interfaces: Vec<RawsockInterface>) {

        // thread::scope(|s| {
        //     for i in &interfaces {
        //         s.spawn(move |_| {
        //             i.start_loop()
        //         });
        //     }
        // }).unwrap();
    }
    fn create_device(&self, desc: InterfaceDescription) -> Result<RawsockInterface, ErrorWithDesc> {
        let name = &desc.name;
        let interface = match self.lib.open_interface(name) {
            Err(err) => return Err(ErrorWithDesc(Error::RawsockErr(err), desc)),
            Ok(interface) => interface
        };

        let data_link = interface.data_link();
        if let rawsock::DataLink::Ethernet = data_link {} else {
            return Err(ErrorWithDesc(Error::WrongDataLink(data_link), desc));
        }
        let interface = Rc::new(RefCell::new(interface));
        match get_mac(name) {
            Ok(mac) => Ok(RawsockInterface {
                shit: false,
                data_link,
                desc,
                interface,
                mac,
            }),
            Err(err) => Err(ErrorWithDesc(Error::GetAddr(err), desc))
        }
    }
}

// unsafe impl<'a> Sync for RawsockInterface<'a> {}
// unsafe impl<'a> Send for RawsockInterface<'a> {}

unsafe impl Sync for RawsockInterface {}
unsafe impl Send for RawsockInterface {}

impl RawsockInterface {
    pub fn name(&self) -> &String {
        &self.desc.name
    }
    pub fn mac(&self) -> &EthernetAddress {
        &self.mac
    }
    pub fn data_link(&self) -> rawsock::DataLink {
        self.data_link
    }
    pub fn start_loop(&self) {
    }
}

pub struct RawRxToken(rawsock::OwnedPacket);

impl RxToken for RawRxToken {
    fn consume<R, F>(self, _timestamp: Instant, f: F) -> smoltcp::Result<R>
        where F: FnOnce(&[u8]) -> smoltcp::Result<R>
    {
        let p = &self.0;
        let len = p.len();
        let result = f(p);
        result
    }
}


pub struct RawTxToken(Rc<RefCell<Box<Interface<'static>>>>);

impl<'a> TxToken for RawTxToken {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> smoltcp::Result<R>
        where F: FnOnce(&mut [u8]) -> smoltcp::Result<R>
    {
        let mut buffer = Vec::new();
        buffer.resize(len, 0);
        let result = f(&mut buffer);
        let interface = self.0;
        let sent = interface.borrow_mut().send(&buffer);
        if !sent.is_ok() {
            println!("send failed {}", len);
        }
        result
    }
}

impl<'d> smoltcp::phy::Device<'d> for RawsockInterface {
    type RxToken = RawRxToken;
    type TxToken = RawTxToken;

    fn receive(&'d mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        self.shit = !self.shit;
        if self.shit {
            None
        } else {
            match self.interface.borrow_mut().receive() {
                Ok(packet) => Some((RawRxToken(packet.into_owned()),
                RawTxToken(self.interface.clone())
                )),
                Err(_) => None
            }
        }
    }

    fn transmit(&'d mut self) -> Option<Self::TxToken> {
        Some(RawTxToken(self.interface.clone()))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1536;
        caps.max_burst_size = Some(1);
        caps
    }
}
