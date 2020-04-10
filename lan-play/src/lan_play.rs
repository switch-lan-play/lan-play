use crate::proxy::Proxy;
use crate::error::{Error, Result};
use crate::rawsock_socket::{ErrorWithDesc, RawsockInterfaceSet, RawsockInterface};
use tokio::task;

pub struct LanPlay<P> {
    proxy: P,
}

#[async_trait(?Send)]
pub trait LanPlayMain {
    async fn start(&mut self, set: &RawsockInterfaceSet) -> Result<()>;
}

impl<P> LanPlay<P>
where
    P: Proxy + 'static
{
    pub async fn new(proxy: P) -> Result<Box<dyn LanPlayMain>> {
        let ret: Box<dyn LanPlayMain> = Box::new(Self {
            proxy,
        });
        Ok(ret)
    }
}

async fn process_interface(mut interf: RawsockInterface) {
    // {
    //     let mut tcp_listener = TcpListener::new(&mut interf).await.unwrap();
    //     while let Ok(Some(socket)) = tcp_listener.next().await {
    //         println!("new connection");
    //     }
    // }
    (&mut interf.running).await;
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
            handles.push(task::spawn(process_interface(interface)));
        }
        for t in handles {
            t.await;
        }

        Ok(())
    }
}
