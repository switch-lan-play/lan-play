use crate::interface_info::{get_interface_info, InterfaceInfo};
use rawsock::traits::{DynamicInterface, Library};
use rawsock::InterfaceDescription;
use smoltcp::wire::{EthernetAddress, Ipv4Cidr};
use std::thread;
use super::device::Packet;
use super::{Error, ErrorWithDesc};
use std::ffi::CString;
use tokio::sync::mpsc::{channel, Sender, Receiver};
use tokio::task;
use tokio::stream::Stream;

type Interface = std::sync::Arc<dyn DynamicInterface<'static> + 'static>;
pub struct RawsockInterface {
    pub desc: InterfaceDescription,
    mac: EthernetAddress,
    data_link: rawsock::DataLink,

    stream: Receiver<Packet>,
    sink: Sender<Packet>,

    pub running: task::JoinHandle<()>,
}

impl Stream for RawsockInterface {
    type Item = Packet;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let s = self.get_mut();
        s.stream.poll_recv(cx)
    }
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

        let (packet_sender, stream) = channel::<Packet>(2);
        let (sink, packet_receiver) = channel::<Packet>(2);

        Self::start_thread(interface.clone(), packet_sender);
        let running = task::spawn(Self::run(
            interface,
            packet_receiver,
        ));

        Ok(RawsockInterface {
            data_link,
            desc: desc.clone(),
            mac,
            running,
            stream,
            sink,
        })
    }
    pub async fn send(&mut self, packet: Packet) -> Result<(), Error> {
        self.sink.send(packet).await.map_err(|e| Error::Other("Send error".into()))
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
        mut packet_receiver: Receiver<Packet>,
    ) {
        loop {
            let dat = packet_receiver.recv().await;
            if let Some(data) = dat {
                let _ = interface.send(&data);
            }
        }
    }
    fn start_thread(interface: Interface, mut packet_sender: Sender<Packet>) {
        log::debug!("recv thread start");
        thread::spawn(move || {
            let r = interface.loop_infinite_dyn(&|packet| {
                match futures::executor::block_on(packet_sender.send(packet.as_owned().to_vec())) {
                    Ok(_) => {},
                    Err(err) => log::warn!("recv error: {:?}", err)
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
    pub fn new(lib: &'static Box<dyn Library>, ip: Ipv4Cidr) -> Result<RawsockInterfaceSet, rawsock::Error> {
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
            errored.into_iter().map(|i| i.err().unwrap()).collect::<Vec<_>>()
        )
    }
    fn open_interface(&self, mut desc: InterfaceDescription) -> Result<RawsockInterface, ErrorWithDesc> {
        RawsockInterface::new(self, &mut desc).map_err(|err| { ErrorWithDesc(err, desc) })
    }
}
