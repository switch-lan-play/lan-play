use crate::get_addr::{get_mac, GetAddressError};
use smoltcp::phy::{DeviceCapabilities,RxToken,TxToken};
use rawsock::traits::{Interface, Library};
use rawsock::InterfaceDescription;
use crossbeam_utils::thread;
use std::cell::RefCell;
use std::rc::Rc;
use smoltcp::{
    iface::{EthernetInterfaceBuilder, NeighborCache, EthernetInterface},
    wire::{IpAddress, IpCidr, EthernetAddress},
    socket::{SocketSet, TcpSocket, TcpSocketBuffer},
    time::{Instant, Duration},
};
use std::collections::BTreeMap;
use crate::duplex::{ChannelPort, Sender};

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
    lib: &'static Box<dyn Library>,
    all_interf: Vec<rawsock::InterfaceDescription>,
    ip: smoltcp::wire::IpCidr,
}

pub struct RawsockDevice {
    shit: bool,
    interface: Rc<RefCell<Box<dyn Interface<'static>>>>,
    port: ChannelPort<Packet>,
}

pub struct RawsockInterface {
    pub desc: InterfaceDescription,
    mac: EthernetAddress,
    data_link: rawsock::DataLink,
    device: RawsockDevice,
    port: ChannelPort<Packet>,
    // dummy: &'a (),
}

impl RawsockInterfaceSet {
    pub fn new(lib: &'static Box<dyn Library>) -> Result<RawsockInterfaceSet, rawsock::Error> {
        let all_interf = lib.all_interfaces()?;
        Ok(RawsockInterfaceSet {
            lib,
            all_interf,
            ip: IpCidr::new(IpAddress::v4(192, 168, 23, 20), 24),
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
        let mut sockets = SocketSet::new(vec![]);
        let interfs = interfaces.into_iter().map(|i| { self.make_iface(i) });
        thread::scope(move |s| {
            for i in interfs {
                s.spawn(move |_| {
                    
                });
            }
        }).unwrap();
    }
    fn make_iface<'a, 'b, 'c>(&self, interf: RawsockInterface) -> EthernetInterface<'a, 'b, 'c, RawsockDevice> {
        let device = interf.device;
        let ethernet_addr = interf.mac().clone();
        let neighbor_cache = NeighborCache::new(BTreeMap::new());
        let ip_addrs = [
            self.ip,
        ];
        let mut iface = EthernetInterfaceBuilder::new(device)
                .ethernet_addr(ethernet_addr)
                .neighbor_cache(neighbor_cache)
                .ip_addrs(ip_addrs)
                .finalize();
        iface
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

        let (port1, port2) = ChannelPort::new();

        match get_mac(name) {
            Ok(mac) => Ok(RawsockInterface {
                data_link,
                desc,
                port: port1,
                device: RawsockDevice {
                    shit: true,
                    interface,
                    port: port2
                },
                mac,
            }),
            Err(err) => Err(ErrorWithDesc(Error::GetAddr(err), desc))
        }
    }
}

unsafe impl Sync for RawsockDevice {}
unsafe impl Send for RawsockDevice {}

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

pub struct RawRxToken(Packet);

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
