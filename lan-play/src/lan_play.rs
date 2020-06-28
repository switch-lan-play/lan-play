use crate::error::{Error, Result};
use crate::future_smoltcp::Net;
use crate::gateway::Gateway;
use crate::proxy::BoxProxy;
use crate::rawsock_socket::{ErrorWithDesc, RawsockInterface, RawsockInterfaceSet};
use futures::future::join_all;
use smoltcp::wire::{Ipv4Address, Ipv4Cidr};

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
    pub async fn start(&mut self, set: &RawsockInterfaceSet, netif: Option<String>) -> Result<()> {
        let (mut opened, errored) = set.open_all_interface();

        for ErrorWithDesc(err, desc) in errored {
            log::warn!(
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
            println!(
                "Interface {} ({}) opened, mac: {}, data link: {}",
                interface.name(),
                interface.desc.description,
                interface.mac(),
                interface.data_link()
            );
        }

        let futures = opened
            .into_iter()
            .map(|interface| self.process_interface(interface))
            .collect::<Vec<_>>();
        join_all(futures).await;

        Ok(())
    }
    async fn process_interface(&self, interf: RawsockInterface) {
        let mac = interf.mac();
        let net = Net::new(
            mac.clone(),
            vec![self.ipv4cidr.into()],
            self.gateway_ip,
            interf,
        );
        let tcp = net.tcp_listener().await;
        let udp = net.udp_socket().await;
        if let Err(err) = self.gateway.process(tcp, udp).await {
            log::error!("gateway::process failed {:?}", err);
        }
        println!("process_interface done");
    }
}
