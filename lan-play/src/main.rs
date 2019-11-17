#[macro_use] extern crate cfg_if;
#[macro_use] extern crate futures;
#[macro_use] extern crate lazy_static;

mod rawsock_socket;
mod interface_info;
mod channel_port;

use futures::{StreamExt, future};
use std::future::Future;
use futures::future::{join_all, join};
use rawsock_socket::{ErrorWithDesc, RawsockInterfaceSet};
use smoltcp::{
    iface::{EthernetInterfaceBuilder},
    socket::{TcpSocket, TcpSocketBuffer, SocketSet},
    wire::{Ipv4Cidr, Ipv4Address}
};
use rawsock::traits::Library;
use async_std::task::{self, JoinHandle};

lazy_static! {
    static ref RAWSOCK_LIB: Box<dyn Library> = {
        let lib = open_best_library().expect("Can't open any library");
        println!("Library opened, version is {}", lib.version());
        lib
    };
}

pub fn open_best_library() -> Result<Box<dyn Library>, rawsock::Error> {
    if let Ok(l) = rawsock::wpcap::Library::open_default_paths() {
        return Ok(Box::new(l));
    }
    match rawsock::pcap::Library::open_default_paths() {
        Ok(l) => Ok(Box::new(l)),
        Err(e) => Err(e)
    }
}

async fn run_interfaces() {
    let set = RawsockInterfaceSet::new(&RAWSOCK_LIB,
        Ipv4Cidr::new(Ipv4Address::new(10, 13, 37, 2), 16),
    ).expect("Could not open any packet capturing library");

    let (mut opened, errored) = set.open_all_interface();

    for ErrorWithDesc(err, desc) in errored {
        log::warn!("Err: Interface {:?} ({:?}) err {:?}", desc.name, desc.description, err);
    }

    for interface in &opened {
        println!("Interface {} ({}) opened, mac: {}, data link: {}", interface.name(), interface.desc.description, interface.mac(), interface.data_link());
    }

    for interface in &mut opened {
        (&mut interface.running).await;
    }
}

fn main() {
    env_logger::init();

    task::block_on(run_interfaces());
}
