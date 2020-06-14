use crate::proxy::{Proxy, BoxProxy, BoxUdp};
use crate::error::{Error, Result};
use crate::rawsock_socket::{ErrorWithDesc, RawsockInterfaceSet, RawsockInterface};
use crate::future_smoltcp::{Net, Socket, NetEvent};
use tokio::task;
use smoltcp::{
    wire::{Ipv4Cidr, Ipv4Address, IpEndpoint}
};
use std::sync::Arc;
use derive_builder::Builder;
use lru::LruCache;

#[derive(Builder)]
pub struct LanPlay {
    proxy: Arc<BoxProxy>,
    ipv4cidr: Ipv4Cidr,
    gateway_ip: Ipv4Address,
    
    #[builder(default = "LruCache::new(100)", setter(skip))]
    cache: LruCache<IpEndpoint, BoxUdp>,
}

impl LanPlay
{
    pub async fn start(&mut self, set: &RawsockInterfaceSet, netif: Option<String>) -> Result<()> {
        let (mut opened, errored) = set.open_all_interface();
    
        for ErrorWithDesc(err, desc) in errored {
            log::warn!("Err: Interface {:?} ({:?}) err {:?}", desc.name, desc.description, err);
        }

        if let Some(netif) = netif {
            opened = opened
                .into_iter()
                .filter(|i| i.name() == &netif)
                .collect();
        }
    
        if opened.len() == 0 {
            return Err(Error::NoInterface)
        }
    
        for interface in &opened {
            println!("Interface {} ({}) opened, mac: {}, data link: {}", interface.name(), interface.desc.description, interface.mac(), interface.data_link());
        }
    
        let mut handles: Vec<task::JoinHandle<()>> = vec![];
        for interface in opened {
            handles.push(task::spawn(process_interface(interface, self.ipv4cidr, self.gateway_ip, self.proxy.clone())));
        }
        for t in handles {
            t.await.map_err(|e| Error::Other(format!("Join error {:?}", e)))?;
        }

        Ok(())
    }
}

async fn process_interface(interf: RawsockInterface, ipv4cidr: Ipv4Cidr, gateway_ip: Ipv4Address, proxy: Arc<BoxProxy>) {
    let mac = interf.mac();
    let net = Net::new(
        mac.clone(), 
        vec![ipv4cidr.into()],
        gateway_ip,
        interf
    );
    let mut udp = net.udp_socket().await;
    loop {
        let result = udp.recv().await;
        println!("udp: {:?}", result);
        if let Ok(udp) = result {
            let mut pudp = proxy.new_udp("0.0.0.0:0".parse().unwrap()).await.unwrap();
            pudp.send_to(&udp.data, udp.dst()).await.unwrap();
        }
    }
    // while let Some(event) = net.next_event().await {
    //     println!("New event {:?}", event);
    //     match event {
    //         NetEvent::Tcp(mut tcp) => {
    //             let _ = tokio::spawn(async move {
    //                 let byte = tcp.read_u8().await.unwrap();
    //                 println!("{:?}: {}", tcp, byte);
    //                 tcp.write_u8(byte).await.unwrap();
    //             });
    //         }
    //         NetEvent::Udp(udp) => {
    //             println!("udp {:?}", udp);
    //         }
    //     };
    // }
    println!("process_interface done");
}
