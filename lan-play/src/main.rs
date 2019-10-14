#[macro_use] extern crate cfg_if;
#[macro_use] extern crate futures;
#[macro_use] extern crate lazy_static;

mod rawsock_socket;
mod interface_info;
mod channel_port;

use tokio::net::signal;
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

lazy_static! {
    static ref RAWSOCK_LIB: Box<dyn Library> = {
        open_best_library().expect("Can't open any library")
        println!("Library opened, version is {}", set.lib_version());
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

    let mut futures: Vec<_> = vec![];
    for interface in &mut opened {
        let name = interface.name().clone();
        println!("Interface {} ({}) opened, mac: {}, data link: {}", name, interface.desc.description, interface.mac(), interface.data_link());
        let future = interface.run();
        futures.push(future);
    }

    join_all(futures).await;

    // let mut tcp2_active = false;
    // set.start(&mut sockets, opened, &mut move |sockets| {
    //     {
    //         let mut socket = sockets.get::<TcpSocket>(tcp2_handle);

    //         if !socket.is_open() {
    //             socket.listen(1234).expect("can not listen to 1234");
    //             socket.set_keep_alive(Some(Duration::from_millis(1000)));
    //             socket.set_timeout(Some(Duration::from_millis(2000)));
    //         }

    //         if socket.is_active() && !tcp2_active {
    //             println!("tcp:1234 connected");
    //         } else if !socket.is_active() && tcp2_active {
    //             println!("tcp:1234 disconnected");
    //         }
    //         tcp2_active = socket.is_active();


    //         if socket.may_recv() {
    //             let data = socket.recv(|buffer| {
    //                 let mut data = buffer.to_owned();
    //                 if data.len() > 0 {
    //                     println!("tcp:1234 recv data: {:?}",
    //                            str::from_utf8(data.as_ref()).unwrap_or("(invalid utf8)"));
    //                     data = data.split(|&b| b == b'\n').collect::<Vec<_>>().concat();
    //                     data.reverse();
    //                     data.extend(b"\n");
    //                 }
    //                 (data.len(), data)
    //             }).unwrap();
    //             if socket.can_send() && data.len() > 0 {
    //                 println!("tcp:1234 send data: {:?}",
    //                        str::from_utf8(data.as_ref()).unwrap_or("(invalid utf8)"));
    //                 socket.send_slice(&data[..]).unwrap();
    //             }
    //         }

    //         println!("  state: {}", socket.state());
    //     }
    // });
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    run_interfaces().await;

    Ok(())
}
