use crate::interface_info::{get_interface_info, InterfaceInfo};
use rawsock::traits::{DynamicInterface, Library};
use rawsock::InterfaceDescription;
use smoltcp::{
    iface::{EthernetInterfaceBuilder, NeighborCache, EthernetInterface},
    wire::{EthernetAddress, Ipv4Cidr},
    socket::{SocketSet, SocketHandle, AnySocket},
    time::{Instant},
};
use std::collections::BTreeMap;
use std::thread;
use super::device::{FutureDevice, Packet, AsyncIface};
use log::{warn, debug};
use super::{Error, ErrorWithDesc};
use std::ffi::CString;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use futures::channel::mpsc::{channel, Sender, Receiver};
use async_std::task;
use futures::select;
use futures::prelude::*;
use async_std::sync::{Arc, RwLock};

use smoltcp::{
    socket::{TcpSocket, TcpSocketBuffer},
};

type Interface = std::sync::Arc<dyn DynamicInterface<'static> + 'static>;
type DeviceType = FutureDevice<Receiver<Packet>, Sender<Packet>>;
type IFace = EthernetInterface<'static, 'static, 'static, DeviceType>;
pub type SharedSockets = Arc<RwLock<SocketSet<'static, 'static, 'static>>>;
pub struct RawsockInterface {
    pub desc: InterfaceDescription,
    mac: EthernetAddress,
    data_link: rawsock::DataLink,

    sockets: SharedSockets,
    pulse_sender: Sender<()>,

    pub running: task::JoinHandle<()>,
}

impl RawsockInterface {
    fn new(slf: &RawsockInterfaceSet, desc: &mut InterfaceDescription) -> Result<RawsockInterface, Error> {
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
        let (pulse_sender, pulse_receiver) = channel(1);
        let device = task::block_on(FutureDevice::new(stream, sink));

        let iface = EthernetInterfaceBuilder::new(device)
                .ethernet_addr(mac.clone())
                .neighbor_cache(neighbor_cache)
                .ip_addrs(ip_addrs)
                .finalize();
        let sockets = Arc::new(RwLock::new(SocketSet::new(vec![])));

        Self::start_thread(interface.clone(), packet_sender);
        let running = task::spawn(Self::run(
            interface,
            iface,
            sockets.clone(),
            packet_receiver,
            pulse_receiver,
        ));

        Ok(RawsockInterface {
            data_link,
            desc: desc.clone(),
            mac,
            sockets,
            pulse_sender,
            running,
        })
    }
    pub fn clone_sockets(&self) -> SharedSockets {
        self.sockets.clone()
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
    async fn run(
        interface: Interface,
        mut iface: EthernetInterface<'static, 'static, 'static, DeviceType>,
        sockets: SharedSockets,
        mut packet_receiver: Receiver<Packet>,
        mut pulse_receiver: Receiver<()>,
    ) {
        loop {
            {
                let mut sockets = sockets.write().await;
                select! {
                    _ = iface.poll_async(&mut sockets).fuse() => {
                        //
                    },
                    dat = packet_receiver.next().fuse() => {
                        if let Some(data) = dat {
                            let _ = interface.send(&data);
                        }
                    },
                    _ = pulse_receiver.next().fuse() => {
                        drop(sockets);
                        pulse_receiver.next().await;
                        // just to do next loop
                    }
                }
            }
        }
    }
    fn start_thread(interface: Interface, mut packet_sender: Sender<Packet>) {
        debug!("recv thread start");
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
    pub(super) async fn borrow_socket<T, F, R>(&self, handle: SocketHandle, f: F) -> R
    where
        T: AnySocket<'static, 'static>,
        F: FnOnce(&mut T) -> R
    {
        self.insert_loop(async {
            let mut sockets = self.sockets.write().await;
            let mut socket = sockets.get::<T>(handle);
            f(&mut socket)
        }).await
    }
    pub(super) async fn new_socket(&self) -> SocketHandle {
        self.insert_loop(async {
            let mut sockets = self.sockets.write().await;
            let socket = TcpSocket::new(
                TcpSocketBuffer::new(vec![0; 2048]),
                TcpSocketBuffer::new(vec![0; 2048])
            );
            sockets.add(socket)
        }).await
    }
    async fn insert_loop<R, F: Future<Output=R>>(&self, future: F) -> R {
        let mut sender = self.pulse_sender.clone();
        sender.send(()).await.unwrap();
        let ret = future.await;
        sender.send(()).await.unwrap();
        ret
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
    fn open_interface(&self, mut desc: InterfaceDescription) -> Result<RawsockInterface, ErrorWithDesc> {
        RawsockInterface::new(self, &mut desc).map_err(|err| { ErrorWithDesc(err, desc) })
    }
}
