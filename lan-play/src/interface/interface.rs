use super::{Error, ErrorWithDesc};
use crate::interface_info::{get_interface_info, InterfaceInfo};
use async_channel::{unbounded, Receiver, Sender};
use rawsock::traits::{DynamicInterface, Library};
use rawsock::InterfaceDescription;
use smoltcp::wire::{EthernetAddress, Ipv4Cidr};
use std::ffi::CString;
use std::sync::Arc;
use std::thread;
use futures::{Stream, Sink};
use std::{pin::Pin, task::{Context, Poll}, io};

pub type Packet = Vec<u8>;
type Interface = Arc<dyn DynamicInterface<'static> + 'static>;

pub struct PacketInterface {
    sink: Sender<Packet>,
    stream: Receiver<Packet>,
}

impl Stream for PacketInterface {
    type Item = Packet;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        Stream::poll_next(Pin::new(&mut self.stream), cx)
    }
}

impl Sink<Packet> for PacketInterface {
    type Error = io::Error;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: Packet) -> Result<(), Self::Error> {
        self.sink.try_send(item).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

pub struct RawsockInterface {
    pub desc: InterfaceDescription,
    mac: EthernetAddress,
    data_link: rawsock::DataLink,
    interface: Arc<dyn DynamicInterface<'static>>,
}

impl std::fmt::Debug for RawsockInterface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawsockInterface")
            .field("desc", &self.desc)
            .field("mac", &self.mac)
            .field("data_link", &self.data_link)
            .finish()
    }
}

impl RawsockInterface {
    fn new(
        slf: &RawsockInterfaceSet,
        desc: &mut InterfaceDescription,
    ) -> Result<RawsockInterface, Error> {
        let name = &desc.name;
        let mut interface = slf.lib.open_interface_arc(name)?;
        Arc::get_mut(&mut interface)
            .unwrap()
            .set_filter_cstr(&slf.filter)?;

        let data_link = interface.data_link();
        if let rawsock::DataLink::Ethernet = data_link {
        } else {
            return Err(Error::WrongDataLink(data_link));
        }
        let InterfaceInfo {
            ethernet_address: mac,
            name: _,
            description,
        } = get_interface_info(name)?;
        if let Some(description) = description {
            desc.description = description;
        }

        Ok(RawsockInterface {
            data_link,
            desc: desc.clone(),
            mac,
            interface,
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
    pub fn start(
        self,
    ) -> PacketInterface {
        let interface = self.interface;
        let (packet_sender, stream) = unbounded();
        let (sink, packet_receiver) = unbounded();

        Self::start_thread(interface.clone(), packet_sender);
        tokio::spawn(Self::run(interface, packet_receiver));

        PacketInterface {
            sink,
            stream,
        }
    }
    async fn run(interface: Interface, packet_receiver: Receiver<Packet>) {
        while let Ok(data) = packet_receiver.recv().await {
            if let Err(e) = interface.send(&data) {
                log::error!("Failed when sending packet {:?}", e);
            }
        }
    }
    fn start_thread(
        interface: Interface,
        packet_sender: Sender<Packet>,
    ) {
        thread::spawn(move || {
            let r = interface.loop_infinite_dyn(&|packet| {
                if let Err(err) = packet_sender.try_send(packet.to_vec()) {
                    log::warn!("recv error: {:?}", err);
                }
            });
            if !r.is_ok() {
                log::warn!("loop_infinite {:?}", r);
            }
            log::debug!("recv thread exit");
        });
    }
}

pub struct RawsockInterfaceSet {
    lib: &'static Box<dyn Library>,
    all_interf: Vec<rawsock::InterfaceDescription>,
    filter: CString,
}
impl RawsockInterfaceSet {
    pub fn new(
        lib: &'static Box<dyn Library>,
        ip: Ipv4Cidr,
    ) -> Result<RawsockInterfaceSet, rawsock::Error> {
        let all_interf = lib.all_interfaces()?;
        let filter = format!("net {}", ip.network());
        log::debug!("filter: {}", filter);
        Ok(RawsockInterfaceSet {
            lib,
            all_interf,
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
            errored
                .into_iter()
                .map(Result::unwrap_err)
                .collect::<Vec<_>>(),
        )
    }
    fn open_interface(
        &self,
        mut desc: InterfaceDescription,
    ) -> Result<RawsockInterface, ErrorWithDesc> {
        RawsockInterface::new(self, &mut desc).map_err(|err| ErrorWithDesc(err, desc))
    }
}
