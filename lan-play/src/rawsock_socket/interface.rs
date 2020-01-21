use crate::interface_info::{get_interface_info, InterfaceInfo};
use rawsock::traits::{DynamicInterface, Library};
use rawsock::InterfaceDescription;
use smoltcp::{
    iface::{EthernetInterfaceBuilder, NeighborCache, EthernetInterface},
    wire::{EthernetAddress, Ipv4Cidr},
    socket::{SocketSet, SocketHandle, AnySocket},
};
use std::collections::BTreeMap;
use std::thread;
use super::device::{FutureDevice, Packet, AsyncIface};
use log::{warn, debug};
use super::{Error, ErrorWithDesc};
use std::ffi::CString;
use std::future::Future;
use futures::channel::mpsc::{channel, Sender, Receiver};
use async_std::task;
use futures::select;
use futures::prelude::*;
use async_std::sync::{Arc, RwLock, Mutex};
use super::socket::AsyncSocket;
use super::event::Event;
use super::net::Net;
use smoltcp::{
    socket::{TcpSocket, TcpSocketBuffer},
};

struct Notify {
    handle: SocketHandle,
    can_read: Sender<()>,
    can_write: Sender<()>,
}


type Interface = std::sync::Arc<dyn DynamicInterface<'static> + 'static>;
pub struct RawsockInterface {
    pub desc: InterfaceDescription,
    mac: EthernetAddress,
    data_link: rawsock::DataLink,

    waiter: Arc<Mutex<Vec<Notify>>>,

    pub running: task::JoinHandle<()>,
}

async fn process_iface(
    interface: Interface,
    mut net: Net,
    mut packet_receiver: Receiver<Packet>,
    waiter: Arc<Mutex<Vec<Notify>>>,
) {
    loop {
        select! {
            _ = net.wait().fuse() => {
                let sockets = net.borrow_sockets();
                let mut waiter = waiter.lock().await;
                println!("poll1 {}", waiter.len());
                for w in &mut waiter.iter_mut() {
                    let s = sockets.get::<TcpSocket>(w.handle);
                    if s.can_recv() {
                        if let Err(e) = w.can_write.send(()).await {
                            println!("send can recv {:?}", e);
                        }
                    }
                };
                println!("poll2 {}", waiter.len());
            },
            dat = packet_receiver.next().fuse() => {
                if let Some(data) = dat {
                    let _ = interface.send(&data);
                }
            },
        }
    }
    println!("process_iface loop stop");
}

impl RawsockInterface {
    fn new(slf: &RawsockInterfaceSet, desc: &mut InterfaceDescription) -> super::Result<RawsockInterface> {
        let name = &desc.name;
        let mut interface = slf.lib.open_interface_arc(name)?;
        std::sync::Arc::get_mut(&mut interface).ok_or(Error::Other("Bad Arc"))?.set_filter_cstr(&slf.filter)?;

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

        let neighbor_cache = NeighborCache::new(BTreeMap::new());
        let ip_addrs = [
            slf.ip.into(),
        ];
        let (packet_sender, stream) = channel::<Packet>(2);
        let (sink, packet_receiver) = channel::<Packet>(2);
        let (event_sender, event_receiver) = channel::<Event>(1);
        let device = task::block_on(FutureDevice::new(stream, sink));

        let iface = EthernetInterfaceBuilder::new(device)
                .ethernet_addr(mac.clone())
                .neighbor_cache(neighbor_cache)
                .ip_addrs(ip_addrs)
                .finalize();
        let net = Net::new(iface, 1);

        let waiter = Arc::new(
            Mutex::new(
                Vec::new()
            )
        );
        Self::start_thread(interface.clone(), packet_sender);
        let running = task::spawn(process_iface(
            interface,
            net,
            packet_receiver,
            event_receiver,
            waiter.clone(),
        ));

        Ok(RawsockInterface {
            data_link,
            desc: desc.clone(),
            mac,
            running,
            waiter,
        })
    }
    pub fn name(&self) -> &String {
        &self.desc.name
    }
    pub fn mac(&self) -> &EthernetAddress {
        &self.mac
    }
    pub fn data_link(&self) -> rawsock::DataLink {
        self.data_link
    }
    fn start_thread(interface: Interface, mut packet_sender: Sender<Packet>) {
        thread::spawn(move || {
            let r = interface.loop_infinite_dyn(&|packet| {
                match task::block_on(packet_sender.send(packet.as_owned().to_vec())) {
                    Ok(_) => (),
                    Err(err) => warn!("recv error: {:?}", err)
                }
            });
            if !r.is_ok() {
                warn!("loop_infinite {:?}", r);
            }
            debug!("recv thread exit");
        });
    }
}

pub struct RawsockInterfaceSet {
    lib: &'static Box<dyn Library>,
    all_interf: Vec<rawsock::InterfaceDescription>,
    ip: Ipv4Cidr,
    filter: CString,
}
impl RawsockInterfaceSet {
    pub fn new(lib: &'static Box<dyn Library>, ip: Ipv4Cidr) -> Result<RawsockInterfaceSet, rawsock::Error> {
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
    fn open_interface(&self, mut desc: InterfaceDescription) -> std::result::Result<RawsockInterface, ErrorWithDesc> {
        RawsockInterface::new(self, &mut desc).map_err(|err| { ErrorWithDesc(err, desc) })
    }
}
