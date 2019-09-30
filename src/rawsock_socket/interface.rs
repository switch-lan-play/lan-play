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
use std::ffi::{CStr, CString};
use super::device::{ChannelDevice, Packet};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use smoltcp::{
    socket::{TcpSocket, TcpSocketBuffer},
};

pub struct RawsockInterfaceSet {
    lib: Box<dyn Library>,
    all_interf: Vec<rawsock::InterfaceDescription>,
    ip: Ipv4Cidr,
    filter: CString,
}

pub struct RawsockInterface<'a, 'b: 'a> {
    pub desc: InterfaceDescription,
    mac: EthernetAddress,
    data_link: rawsock::DataLink,

    interface: Arc<dyn DynamicInterface<'a> + 'a>,
    iface: Option<EthernetInterface<'a, 'a, 'a, ChannelDevice>>,

    port: ChannelPort<Packet>,
    recv_thread: Option<JoinHandle<()>>,

    _dummy: &'b std::marker::PhantomData<()>,
}

impl<'a> RawsockInterfaceSet {
    pub fn new(lib: Box<dyn Library>, ip: Ipv4Cidr) -> Result<RawsockInterfaceSet, rawsock::Error> {
        let all_interf = lib.all_interfaces()?;
        let filter = format!("net {}", ip.network());
        debug!("filter: {}", filter);
        Ok(RawsockInterfaceSet {
            lib,
            all_interf,
            ip,
            filter: CString::new(filter)?,
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
    fn open_interface(&self, desc: InterfaceDescription) -> Result<RawsockInterface, ErrorWithDesc> {
        self.open_interface_inner(&desc).map_err(|err| { ErrorWithDesc(err, desc) })
    }
    fn open_interface_inner(&self, desc: &InterfaceDescription) -> Result<RawsockInterface, Error> {
        let name = &desc.name;
        let mut interface = self.lib.open_interface_arc(name)?;
        // Arc::get_mut(&mut interface).ok_or(Error::Other("Bad Arc"))?.set_filter_cstr(&self.filter)?;

        let data_link = interface.data_link();
        if let rawsock::DataLink::Ethernet = data_link {} else {
            return Err(Error::WrongDataLink(data_link));
        }
        let mac = get_mac(name)?;
        
        let (port1, port2) = ChannelPort::new();
        let device = ChannelDevice::new(port2);

        let ethernet_addr = mac.clone();
        let neighbor_cache = NeighborCache::new(BTreeMap::new());
        let ip_addrs = [
            self.ip.into(),
        ];
        let iface = Some(EthernetInterfaceBuilder::new(device)
                .ethernet_addr(ethernet_addr)
                .neighbor_cache(neighbor_cache)
                .ip_addrs(ip_addrs)
                .finalize());

        Ok(RawsockInterface {
            data_link,
            desc: desc.clone(),
            mac,
            interface,
            port: port1,
            iface,
            recv_thread: None,
            _dummy: &std::marker::PhantomData,
        })
    }
}

struct Shit<'a, 'b: 'a, 'c: 'a> {
    iface: &'a mut EthernetInterface<'b, 'b, 'b, ChannelDevice>,
    sockets: &'a mut SocketSet<'c, 'c, 'c>,
}

impl Future for Shit<'_, '_, '_> {
    type Output = smoltcp::Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let shit = self.get_mut();
        let timestamp = Instant::now();
        println!("begin poll");
        let ret = match shit.iface.poll(&mut shit.sockets, timestamp) {
            Ok(true) => Poll::Ready(Ok(())),
            Ok(false) => Poll::Pending,
            Err(e) => Poll::Ready(Err(e)),
        };
        println!("end poll");
        ret
    }
}

impl<'b, 'a: 'b> RawsockInterface<'a, 'b> {
    pub fn name(&self) -> &String {
        &self.desc.name
    }
    pub fn mac(&self) -> &EthernetAddress {
        &self.mac
    }
    pub fn data_link(&self) -> rawsock::DataLink {
        self.data_link
    }
    pub async fn run(&mut self) {
        self.start_thread();
        let mut sockets = SocketSet::new(vec![]);
        let mut listening_socket = new_listening_socket();
        let listening_socket_handle = sockets.add(listening_socket);
        let mut iface = match self.iface.take() {
            Some(iface) => iface,
            None => return
        };
        loop {
            println!("Shit1");
            Shit{
                iface: &mut iface,
                sockets: &mut sockets,
            }.await.unwrap();
            println!("Shit2");
        }
    }
    fn start_thread(&mut self) {
        if let Some(_) = self.recv_thread {
            return
        }
        let static_self = unsafe{hide_lt(self)};
        let sender = self.port.clone_sender();
        let interf = static_self.interface.clone();
        let recv_thread = Some(spawn(move || {
            let r = interf.loop_infinite_dyn(&|packet| {
                // s.set_readiness(Ready::readable()).unwrap();
                match sender.send(packet.as_owned().to_vec()) {
                    Ok(_) => (),
                    Err(err) => warn!("recv error: {:?}", err)
                }
            });
            if !r.is_ok() {
                warn!("loop_infinite {:?}", r);
            }
            debug!("recv thread exit");
        }));
        self.recv_thread = recv_thread;
    }
}

fn new_listening_socket<'a>() -> TcpSocket<'a> {
    let mut listening_socket = TcpSocket::new(
        TcpSocketBuffer::new(vec![0; 2048]),
        TcpSocketBuffer::new(vec![0; 2048])
    );
    listening_socket.set_accept_all(true);
    listening_socket
}

unsafe fn hide_lt<'a, 'b>(v: &mut (RawsockInterface<'a, 'b>)) -> &'a mut (RawsockInterface<'static, 'static>) {
    use std::mem;
    mem::transmute(v)
}
