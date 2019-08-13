use std::sync::Arc;
use std::ops::{Deref};
use std::cell::RefCell;
use crate::get_addr::{get_mac, GetAddressError};
use smoltcp::phy::{DeviceCapabilities,RxToken,TxToken};
use rawsock::traits::{DynamicInterface as Interface, Library};
use rawsock::InterfaceDescription;
use crossbeam_utils::{thread, sync::Parker};
use smoltcp::{
    iface::{EthernetInterfaceBuilder, NeighborCache, EthernetInterface},
    wire::{IpCidr, EthernetAddress},
    socket::{SocketSet},
    time::{Instant},
};
use std::collections::BTreeMap;
use crate::duplex::{ChannelPort, Sender};
use log::{warn, debug};

type Packet = Vec<u8>;
#[derive(Debug)]
pub enum Error {
    RawsockErr(rawsock::Error),
    WrongDataLink(rawsock::DataLink),
    GetAddr(GetAddressError),
}
#[derive(Debug)]
pub struct ErrorWithDesc (pub Error, pub InterfaceDescription);

pub struct RawsockInterfaceSet {
    lib: Box<dyn Library>,
    all_interf: Vec<rawsock::InterfaceDescription>,
    ip: smoltcp::wire::IpCidr,
}

pub struct RawsockDevice {
    pub port: ChannelPort<Packet>,
}

pub struct RawsockRunner<'a> {
    pub port: ChannelPort<Packet>,
    pub interface: Arc<InterfaceMT<'a>>,
}

pub struct RawsockInterface<'a> {
    pub desc: InterfaceDescription,
    mac: EthernetAddress,
    data_link: rawsock::DataLink,
    device: RawsockDevice,
    port: ChannelPort<Packet>,
    interface: InterfaceMT<'a>,
    ip: smoltcp::wire::IpCidr,
    // dummy: &'a (),
}

pub struct InterfaceMT<'a> (RefCell<Box<dyn Interface<'a> + 'a>>);
unsafe impl<'a> Sync for InterfaceMT<'a> {}
unsafe impl<'a> Send for InterfaceMT<'a> {}
impl<'a> Deref for InterfaceMT<'a> {
    type Target = RefCell<Box<dyn Interface<'a> + 'a>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl RawsockInterfaceSet {
    pub fn new(lib: Box<dyn Library>, ip: IpCidr) -> Result<RawsockInterfaceSet, rawsock::Error> {
        let all_interf = lib.all_interfaces()?;
        Ok(RawsockInterfaceSet {
            lib,
            all_interf,
            ip,
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

    pub fn start(&self, sockets: &mut SocketSet<'_, '_, '_>, interfaces: Vec<RawsockInterface>, f: &mut dyn FnMut(&mut SocketSet)) {
        let (mut devs, runners): (Vec<_>, Vec<_>) = interfaces
            .into_iter()
            .map(|i| { i.split_iface() })
            .unzip();
        thread::scope(move |s| {
            let parker = Parker::new();
            for runner in runners {
                let sender = runner.port.clone_sender();
                let interf = runner.interface.clone();
                let unparker = parker.unparker().clone();
                s.spawn(move |_| {
                    let r = interf.borrow().loop_infinite_dyn(&|packet| {
                        unparker.unpark();
                        match sender.send(packet.as_owned().to_vec()) {
                            Ok(_) => (),
                            Err(err) => warn!("recv error: {:?}", err)
                        }
                    });
                    if !r.is_ok() {
                        warn!("loop_infinite {:?}", r);
                    }
                    debug!("recv thread exit");
                });
                s.spawn(move |_| {
                    let port = runner.port;
                    let interf = runner.interface.clone();
                    while let Ok(to_send) = port.recv() {
                        match interf.borrow().send(&to_send) {
                            Ok(_) => (),
                            Err(err) => warn!("send error: {:?}", err)
                        }
                    }
                    debug!("send thread exit");
                });
            }
            loop {
                f(sockets);
                parker.park();
                for dev in &mut devs {
                    match dev.poll(sockets, Instant::now()) {
                        Err(smoltcp::Error::Unrecognized) => continue,
                        Err(err) => {
                            println!("poll err {}", err);
                        },
                        Ok(_) => ()
                    }
                }
            }
        }).unwrap();
    }
    fn create_device<'a>(&'a self, desc: InterfaceDescription) -> Result<RawsockInterface<'a>, ErrorWithDesc> {
        let name = &desc.name;
        let interface = match self.lib.open_interface(name) {
            Err(err) => return Err(ErrorWithDesc(Error::RawsockErr(err), desc)),
            Ok(interface) => interface
        };

        let data_link = interface.data_link();
        if let rawsock::DataLink::Ethernet = data_link {} else {
            return Err(ErrorWithDesc(Error::WrongDataLink(data_link), desc));
        }

        let (port1, port2) = ChannelPort::new();
        let interface = InterfaceMT(RefCell::new(interface));

        match get_mac(name) {
            Ok(mac) => Ok(RawsockInterface {
                data_link,
                desc,
                port: port1,
                device: RawsockDevice {
                    port: port2
                },
                mac,
                interface,
                ip: self.ip.clone()
            }),
            Err(err) => Err(ErrorWithDesc(Error::GetAddr(err), desc))
        }
    }
}

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
    pub fn split_device(self) -> (RawsockDevice, RawsockRunner<'a>) {
        (self.device, RawsockRunner {
            port: self.port,
            interface: Arc::new(self.interface)
        })
    }
    pub fn split_iface<'b, 'c, 'e>(self) -> (
            EthernetInterface<'b, 'c, 'e, RawsockDevice>,
            RawsockRunner<'a>
    ) {
        let ethernet_addr = self.mac().clone();
        let device = self.device;
        let interface = Arc::new(self.interface);
        let neighbor_cache = NeighborCache::new(BTreeMap::new());
        let ip_addrs = [
            self.ip,
        ];
        let iface = EthernetInterfaceBuilder::new(device)
                .ethernet_addr(ethernet_addr)
                .neighbor_cache(neighbor_cache)
                .ip_addrs(ip_addrs)
                .finalize();
        (iface, RawsockRunner {
            port: self.port,
            interface
        })
    }
}

pub struct RawRxToken(Packet);

impl RxToken for RawRxToken {
    fn consume<R, F>(self, _timestamp: Instant, f: F) -> smoltcp::Result<R>
        where F: (FnOnce(&[u8]) -> smoltcp::Result<R>)
    {
        let p = &self.0;
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

impl<'d> smoltcp::phy::Device<'d> for RawsockDevice {
    type RxToken = RawRxToken;
    type TxToken = RawTxToken;

    fn receive(&'d mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        match self.port.try_recv() {
            Ok(packet) => Some((
                RawRxToken(packet),
                RawTxToken(self.port.clone_sender())
            )),
            Err(_) => None
        }
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
