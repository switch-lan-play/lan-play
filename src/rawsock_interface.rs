use crate::get_addr::{get_mac, GetAddressError};
use smoltcp::phy::{DeviceCapabilities,RxToken,TxToken};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress};
use rawsock::traits::{Interface, Library};
use rawsock::InterfaceDescription;
use crossbeam_utils::thread;
use smoltcp::{iface::{EthernetInterfaceBuilder, NeighborCache}, wire::{IpAddress, IpCidr}};

#[derive(Debug)]
pub enum Error {
    RawsockErr(rawsock::Error),
    WrongDataLink(rawsock::DataLink),
    GetAddr(GetAddressError),
}
#[derive(Debug)]
pub struct ErrorWithDesc (pub Error, pub InterfaceDescription);

pub struct RawsockInterfaceSet {
    lib:        Box<dyn Library + Send>,
    all_interf: Vec<rawsock::InterfaceDescription>
}

pub struct RawsockInterface<'a> {
    rx_buffer: [u8; 1536],
    tx_buffer: [u8; 1536],
    pub desc: InterfaceDescription,
    interface: Box<dyn Interface<'a> + 'a>,
    mac: EthernetAddress,
    data_link: rawsock::DataLink,
    dummy: &'a (),
}

impl<'a> RawsockInterfaceSet {
    pub fn new() -> Result<RawsockInterfaceSet, rawsock::Error> {
        let lib = rawsock::open_best_library()?;
        let all_interf = lib.all_interfaces()?;
        Ok(RawsockInterfaceSet {
            lib,
            all_interf,
        })
    }
    pub fn lib_version(&self) -> rawsock::LibraryVersion {
        self.lib.version()
    }
    pub fn open_all_interface(&'a self) -> (Vec<RawsockInterface<'a>>, Vec<ErrorWithDesc>) {
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
    pub fn start(&self, interfaces: Vec<RawsockInterface<'a>>) {

        // thread::scope(|s| {
        //     for i in &interfaces {
        //         s.spawn(move |_| {
        //             i.start_loop()
        //         });
        //     }
        // }).unwrap();
    }
    fn create_device(&'a self, desc: InterfaceDescription) -> Result<RawsockInterface<'a>, ErrorWithDesc> {
        let name = &desc.name;
        let interface = self.lib.open_interface(name);
        match interface {
            Err(err) => Err(ErrorWithDesc(Error::RawsockErr(err), desc)),
            Ok(interface) => {
                let data_link = interface.data_link();
                if let rawsock::DataLink::Ethernet = data_link {} else {
                    return Err(ErrorWithDesc(Error::WrongDataLink(data_link), desc));
                }
                // let thread = thread::scope::(|s| {
                //     s.spawn(|_| {
                //         let shit = self.open_interface(name);
                //     });
                // });
                match get_mac(name) {
                    Ok(mac) => Ok(RawsockInterface {
                        rx_buffer: [0; 1536],
                        tx_buffer: [0; 1536],
                        data_link,
                        desc,
                        interface,
                        mac,
                        dummy: &(),
                    }),
                    Err(err) => Err(ErrorWithDesc(Error::GetAddr(err), desc))
                }
            }
        }
    }
}

unsafe impl<'a> Sync for RawsockInterface<'a> {}
unsafe impl<'a> Send for RawsockInterface<'a> {}

impl<'a> RawsockInterface<'a> {
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

pub struct RawRxToken<'a>(&'a mut [u8]);

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

impl<'a, 'b> smoltcp::phy::Device<'a> for RawsockInterface<'a> {
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
