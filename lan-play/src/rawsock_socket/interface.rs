use std::sync::{Arc, Mutex};
use crate::interface_info::{get_interface_info, InterfaceInfo};
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
use std::task::{Context, Poll, Waker};

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
    iface: Option<EthernetInterface<'a, 'a, 'a, ChannelDevice>>,

    port: ChannelPort<Packet>,
    recv_thread: Option<JoinHandle<()>>,

    waker: Arc<Mutex<Option<Waker>>>,
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
    fn open_interface(&self, mut desc: InterfaceDescription) -> Result<RawsockInterface, ErrorWithDesc> {
        self.open_interface_inner(&mut desc).map_err(|err| { ErrorWithDesc(err, desc) })
    }
    fn open_interface_inner(&self, desc: &mut InterfaceDescription) -> Result<RawsockInterface, Error> {
        let name = &desc.name;
        let mut interface = self.lib.open_interface_arc(name)?;
        Arc::get_mut(&mut interface).ok_or(Error::Other("Bad Arc"))?.set_filter_cstr(&self.filter)?;

        let data_link = interface.data_link();
        if let rawsock::DataLink::Ethernet = data_link {} else {
            return Err(Error::WrongDataLink(data_link));
        }
        let InterfaceInfo {
            ethernet_address: mac,
            name: _,
            description
        } = get_interface_info(name)?;
        if let Some(description) = description {
            desc.description = description;
        }
        
        let (port1, port2) = ChannelPort::new();
        let device = ChannelDevice::new(port2);

        let neighbor_cache = NeighborCache::new(BTreeMap::new());
        let ip_addrs = [
            self.ip.into(),
        ];
        let iface = Some(EthernetInterfaceBuilder::new(device)
                .ethernet_addr(mac.clone())
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
            waker: Arc::new(Mutex::new(None)),
        })
    }
}

struct PollSocket<'a, 'b: 'a, 'c: 'a> {
    iface: &'a mut EthernetInterface<'b, 'b, 'b, ChannelDevice>,
    sockets: &'a mut SocketSet<'c, 'c, 'c>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl Future for PollSocket<'_, '_, '_> {
    type Output = smoltcp::Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut waker = this.waker.lock().unwrap();
        waker.replace(cx.waker().clone());
        let timestamp = Instant::now();
        let ret = match this.iface.poll(&mut this.sockets, timestamp) {
            Ok(true) => Poll::Ready(Ok(())),
            Ok(false) => Poll::Pending,
            Err(e) => Poll::Ready(Err(e)),
        };
        ret
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
    pub fn clone_interface(&self) -> Arc<dyn DynamicInterface<'a> + 'a> {
        self.interface.clone()
    }
    pub async fn run(&mut self) {
        self.start_thread();
        let mut sockets = SocketSet::new(vec![]);
        let mut listening_socket_handle = sockets.add(new_listening_socket());
        let mut iface = match self.iface.take() {
            Some(iface) => iface,
            None => return
        };
        loop {
            match (PollSocket{
                iface: &mut iface,
                sockets: &mut sockets,
                waker: self.waker.clone(),
            }.await) {
                Ok(_) => (),
                Err(_) => (),
            };
            while let Ok(data) = self.port.try_recv() {
                self.interface.send(&data);
            }
            let mut need_new_listen = false;
            {
                let mut socket = sockets.get::<TcpSocket>(listening_socket_handle);
                if socket.can_send() {
                    need_new_listen = true;
                    debug!("shit I get it");
                }
            }
            if need_new_listen {
                listening_socket_handle = sockets.add(new_listening_socket());
                sockets.prune();
                debug!("size {}", sockets.iter().count());
            }
        }
    }
    fn start_thread(&mut self) {
        debug!("recv thread start");
        if let Some(_) = self.recv_thread {
            return
        }
        let static_self = unsafe{hide_lt(self)};
        let sender = self.port.clone_sender();
        let interf = static_self.interface.clone();
        let waker = static_self.waker.clone();
        self.recv_thread = Some(spawn(move || {
            let r = interf.loop_infinite_dyn(&|packet| {
                if let Some(waker) = waker.lock().unwrap().take() {
                    waker.wake();
                }
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
    }
}

fn new_listening_socket<'a>() -> TcpSocket<'a> {
    let mut listening_socket = TcpSocket::new(
        TcpSocketBuffer::new(vec![0; 2048]),
        TcpSocketBuffer::new(vec![0; 2048])
    );
    listening_socket.set_accept_all(true);
    listening_socket.listen(1).unwrap();
    listening_socket
}

unsafe fn hide_lt<'a>(v: &mut (RawsockInterface<'a>)) -> &'a mut (RawsockInterface<'static>) {
    use std::mem;
    mem::transmute(v)
}
