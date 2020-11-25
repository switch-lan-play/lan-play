use crate::error::{Error, Result};
use crate::future_smoltcp::{Net, TcpListener, BufferSize};
use crate::gateway::Gateway;
use crate::proxy::BoxedProxy;
use crate::interface::{ErrorWithDesc, RawsockInterface, RawsockInterfaceSet};
use crate::client::LanClient;
use futures::{future::{join_all, ready}, stream::StreamExt};
use smoltcp::wire::{Ipv4Address, Ipv4Cidr, EthernetFrame, EthernetProtocol, Ipv4Packet};

const BACKLOG: usize = 10;

fn filter_bad_packet(packet: &[u8]) -> Result<()> {
    let packet = EthernetFrame::new_checked(packet)?;
    match packet.ethertype() {
        EthernetProtocol::Arp => {},
        EthernetProtocol::Ipv4 => {
            let packet = Ipv4Packet::new_checked(packet.payload())?;
            if !packet.dst_addr().is_unicast() {
                // ignore broadcast?
                return Err(Error::BadPacket)
            }
        },
        _ => return Err(Error::BadPacket),
    };
    
    Ok(())
}

pub struct LanPlay {
    gateway: Gateway,
    ipv4cidr: Ipv4Cidr,
    gateway_ip: Ipv4Address,
    mtu: usize,
    buffer_size: BufferSize,
}

impl LanPlay {
    pub fn new(proxy: BoxedProxy, ipv4cidr: Ipv4Cidr, gateway_ip: Ipv4Address, mtu: usize, buffer_size: BufferSize) -> LanPlay {
        LanPlay {
            gateway: Gateway::new(proxy),
            ipv4cidr,
            gateway_ip,
            mtu,
            buffer_size,
        }
    }
    pub async fn start(&mut self, set: &RawsockInterfaceSet, netif: Option<String>, _client: Option<LanClient>) -> Result<()> {
        let (mut opened, errored) = set.open_all_interface();

        for ErrorWithDesc(err, desc) in errored {
            log::debug!(
                "Err: Interface {:?} ({:?}) err {:?}",
                desc.name,
                desc.description,
                err
            );
        }

        if let Some(netif) = netif {
            opened = opened.into_iter().filter(|i| i.name() == &netif).collect();
        }

        if opened.len() == 0 {
            return Err(Error::NoInterface);
        }

        for interface in &opened {
            log::info!(
                "Interface {} ({}) opened, mac: {}, data link: {}",
                interface.name(),
                interface.desc.description,
                interface.mac(),
                interface.data_link()
            );
        }

        let futures = opened
            .into_iter()
            .map(|interface| {
                self.process_interface(interface)
            })
            .collect::<Vec<_>>();
        join_all(futures).await;

        Ok(())
    }
    async fn process_interface(&self, interf: RawsockInterface) {
        let mac = interf.mac().to_owned();
        let stream = interf.start();
        // TODO: add lan_client
        let stream = stream.filter(|p| ready(filter_bad_packet(p).is_ok()));
        let net = Net::new(
            mac.clone(),
            vec![self.ipv4cidr.into()],
            self.gateway_ip,
            stream,
            self.mtu,
            self.buffer_size,
        );
        let tcp: Vec<TcpListener> = join_all((0..BACKLOG).map(|_| net.tcp_listener())).await;
        let udp = net.udp_socket().await;
        if let Err(err) = self.gateway.process(tcp, udp).await {
            log::error!("gateway::process failed {:?}", err);
        }
    }
}
