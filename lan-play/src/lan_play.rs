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
    proxy: P,
    gateway_ip: Ipv4Address,
}

#[async_trait(?Send)]
pub trait LanPlayMain {
    async fn start(&mut self, set: &RawsockInterfaceSet) -> Result<()>;
}

impl<P> LanPlay<P>
where
    P: Proxy + 'static
{
    pub async fn new(proxy: P, gateway_ip: Ipv4Address) -> Result<Box<dyn LanPlayMain>> {
        let ret: Box<dyn LanPlayMain> = Box::new(Self {
            proxy,
            gateway_ip,
        });
        Ok(ret)
    }
}

async fn process_interface(mut interf: RawsockInterface, gateway_ip: Ipv4Address) {
    // {
    //     let mut tcp_listener = TcpListener::new(&mut interf).await.unwrap();
    //     while let Ok(Some(socket)) = tcp_listener.next().await {
    //         println!("new connection");
    //     }
    // }
    let mac = interf.mac();
    EthernetInterface::new(mac.clone(), vec![gateway_ip], interf);
}

#[async_trait(?Send)]
impl<P> LanPlayMain for LanPlay<P> {
    async fn start(&mut self, set: &RawsockInterfaceSet) -> Result<()> {
        let (opened, errored) = set.open_all_interface();
    
        if opened.len() == 0 {
            return Err(Error::NoInterface)
        }
    
        for ErrorWithDesc(err, desc) in errored {
            log::warn!("Err: Interface {:?} ({:?}) err {:?}", desc.name, desc.description, err);
        }
    
        for interface in &opened {
            println!("Interface {} ({}) opened, mac: {}, data link: {}", interface.name(), interface.desc.description, interface.mac(), interface.data_link());
        }
    
        let mut handles: Vec<task::JoinHandle<()>> = vec![];
        for interface in opened {
            handles.push(task::spawn(process_interface(interface, self.gateway_ip)));
        }
        for t in handles {
            t.await;
        }

        Ok(())
    }
}
