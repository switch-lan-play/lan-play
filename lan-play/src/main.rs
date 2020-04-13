#[macro_use] extern crate cfg_if;
#[macro_use] extern crate futures;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate async_trait;

mod rawsock_socket;
mod interface_info;
mod proxy;
mod lan_play;
mod error;
mod future_smoltcp;

use rawsock_socket::RawsockInterfaceSet;
use smoltcp::{
    wire::{Ipv4Cidr, Ipv4Address}
};
use rawsock::traits::Library;
use lan_play::LanPlay;
use proxy::DirectProxy;
use error::Result;

lazy_static! {
    static ref RAWSOCK_LIB: Box<dyn Library> = {
        let lib = open_best_library().expect("Can't open any library");
        println!("Library opened, version is {}", lib.version());
        lib
    };
}

fn open_best_library() -> Result<Box<dyn Library>> {
    if let Ok(l) = rawsock::wpcap::Library::open_default_paths() {
        return Ok(Box::new(l));
    }
    Ok(Box::new(rawsock::pcap::Library::open_default_paths()?))
}

async fn async_main() -> Result<()> {
    let set = RawsockInterfaceSet::new(&RAWSOCK_LIB,
        Ipv4Cidr::new(Ipv4Address::new(10, 13, 37, 2), 16),
    ).expect("Could not open any packet capturing library");

    let mut lp = Box::new(LanPlay::new(DirectProxy::new()).await.unwrap());

    lp.start(&set).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    async_main().await
}
