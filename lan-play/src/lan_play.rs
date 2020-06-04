use crate::proxy::Proxy;
use crate::error::{Error, Result};
use crate::rawsock_socket::{ErrorWithDesc, RawsockInterfaceSet, RawsockInterface};
use crate::future_smoltcp::EthernetInterface;
use tokio::task;
use tokio::stream::StreamExt;
use smoltcp::{
    wire::{Ipv4Cidr, Ipv4Address}
};

pub struct LanPlay<P> {
    pub proxy: P,
    pub ipv4cidr: Ipv4Cidr,
}

#[async_trait(?Send)]
pub trait LanPlayMain {
    async fn start(&mut self, set: &RawsockInterfaceSet, netif: Option<String>) -> Result<()>;
}

impl<P> LanPlay<P>
where
    P: Proxy + 'static
{
    pub async fn build(self) -> Result<Box<dyn LanPlayMain>> {
        let ret: Box<dyn LanPlayMain> = Box::new(self);
        Ok(ret)
    }
}

async fn process_interface(interf: RawsockInterface, ipv4cidr: Ipv4Cidr) {
    // {
    //     let mut tcp_listener = TcpListener::new(&mut interf).await.unwrap();
    //     while let Ok(Some(socket)) = tcp_listener.next().await {
    //         println!("new connection");
    //     }
    // }
    let mac = interf.mac();
    let mut interf = EthernetInterface::new(mac.clone(), vec![ipv4cidr.into()], interf);
    while let Some(socket) = interf.next_socket().await {
        println!("New socket {:?}", socket);
    }
    println!("process_interface done");
}

#[async_trait(?Send)]
impl<P> LanPlayMain for LanPlay<P> {
    async fn start(&mut self, set: &RawsockInterfaceSet, netif: Option<String>) -> Result<()> {
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
            handles.push(task::spawn(process_interface(interface, self.ipv4cidr)));
        }
        for t in handles {
            t.await.map_err(|e| Error::Other(format!("Join error {:?}", e)))?;
        }

        Ok(())
    }
}
