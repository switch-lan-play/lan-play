use std::sync::Arc;
use crate::get_addr::get_mac;
use rawsock::traits::{DynamicInterface, Library};
use rawsock::InterfaceDescription;
use smoltcp::{
    iface::{EthernetInterfaceBuilder, NeighborCache, EthernetInterface},
    wire::{IpCidr, EthernetAddress, Ipv4Cidr},
    socket::{SocketSet},
    time::{Instant},
};
use std::collections::BTreeMap;
use std::thread::{JoinHandle, spawn};
use crate::channel_port::ChannelPort;
use log::{warn, debug};
use super::{Error, ErrorWithDesc};
use super::device::{RawsockDevice, Packet};
use std::ffi::{CStr, CString};

use smoltcp::{
    socket::{TcpSocket, TcpSocketBuffer},
};

pub struct RawsockInterfaceSet {
    lib: Box<dyn Library>,
    all_interf: Vec<rawsock::InterfaceDescription>,
    ip: Ipv4Cidr,
    filter: CString,
}

pub struct RawsockInterface<'a> {
    pub desc: InterfaceDescription,
    mac: EthernetAddress,
    data_link: rawsock::DataLink,

    interface: Arc<dyn DynamicInterface<'a> + 'a>,
}

impl RawsockInterfaceSet {
    pub fn new(lib: Box<dyn Library>, ip: Ipv4Cidr) -> Result<RawsockInterfaceSet, rawsock::Error> {
        let all_interf = lib.all_interfaces()?;
        let filter = format!("net {}", ip.network());
        debug!("filter: {}", filter);
        Ok(RawsockInterfaceSet {
            lib,
            all_interf,
            ip,
            filter: CString::new(filter)?
        })
    }
    pub fn lib_version(&self) -> rawsock::LibraryVersion {
        self.lib.version()
    }
    pub fn open_all_interface(&self) -> (Vec<RawsockInterface>, Vec<ErrorWithDesc>) {
        let all_interf = self.all_interf.clone();
        let (opened, errored): (Vec<_>, _) = all_interf
            .into_iter()
            .map(|i| self.open_interface(i))
            .partition(Result::is_ok);
        (
            opened.into_iter().map(Result::unwrap).collect::<Vec<_>>(),
            errored.into_iter().map(|i| i.err().unwrap()).collect::<Vec<_>>()
        )
    }
    fn open_interface<'a>(&'a self, desc: InterfaceDescription) -> Result<RawsockInterface<'a>, ErrorWithDesc> {
        self.open_interface_inner(&desc).map_err(|err| { ErrorWithDesc(err, desc) })
    }
    fn open_interface_inner<'a>(&'a self, desc: &InterfaceDescription) -> Result<RawsockInterface<'a>, Error> {
        let name = &desc.name;
        let mut interface = self.lib.open_interface_arc(name)?;
        Arc::get_mut(&mut interface).ok_or(Error::Other("Bad Arc"))?.set_filter_cstr(&self.filter)?;

        let data_link = interface.data_link();
        if let rawsock::DataLink::Ethernet = data_link {} else {
            return Err(Error::WrongDataLink(data_link));
        }
        let mac = get_mac(name)?;

        Ok(RawsockInterface {
            data_link,
            desc: desc.clone(),
            mac,
            interface,
        })
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
    pub fn interface(&self) -> &Arc<dyn DynamicInterface<'a> + 'a> {
        &self.interface
    }
    pub fn interface_mut(&mut self) -> &mut Arc<dyn DynamicInterface<'a> + 'a> {
        &mut self.interface
    }
}
