use crate::error::{Error, Result};
use crate::future_smoltcp::Net;
use crate::gateway::Gateway;
use crate::proxy::BoxProxy;
use crate::interface::{ErrorWithDesc, RawsockInterface, RawsockInterfaceSet, IntercepterBuilder, BorrowedPacket};
use crate::client::LanClient;
use futures::future::join_all;
use smoltcp::wire::{Ipv4Address, Ipv4Cidr, EthernetFrame, EthernetProtocol, Ipv4Packet};

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
}

impl LanPlay {
    pub fn new(proxy: BoxProxy, ipv4cidr: Ipv4Cidr, gateway_ip: Ipv4Address) -> LanPlay {
        LanPlay {
            gateway: Gateway::new(proxy),
            ipv4cidr,
            gateway_ip,
        }
    }
    pub async fn start(&mut self, set: &RawsockInterfaceSet, netif: Option<String>, client: Option<LanClient>) -> Result<()> {
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
                let mut intercepter = IntercepterBuilder::new()
                    .add(|packet| {
                        filter_bad_packet(packet).is_err()
                    });
                if let Some(client) = &client {
                    intercepter = intercepter.add_factory(client.to_intercepter_factory());
                }

                self.process_interface(interface, intercepter)
            })
            .collect::<Vec<_>>();
        join_all(futures).await;

        Ok(())
    }
    async fn process_interface(&self, interf: RawsockInterface, intercepter: IntercepterBuilder) {
        let mac = interf.mac();
        let net = Net::new(
            mac.clone(),
            vec![self.ipv4cidr.into()],
            self.gateway_ip,
            interf,
            intercepter,
        );
        let tcp = net.tcp_listener().await;
        let udp = net.udp_socket().await;
        if let Err(err) = self.gateway.process(tcp, udp).await {
            log::error!("gateway::process failed {:?}", err);
        }
    }
}
