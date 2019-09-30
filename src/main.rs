#[feature("async-await")]
#[macro_use] extern crate cfg_if;
#[macro_use] extern crate futures;

mod rawsock_socket;
mod get_addr;
mod channel_port;

use rawsock_socket::{ErrorWithDesc, RawsockInterfaceSet};
use smoltcp::{
    iface::{EthernetInterfaceBuilder},
    socket::{TcpSocket, TcpSocketBuffer, SocketSet},
    wire::{Ipv4Cidr, Ipv4Address}
};

async fn fuck() {
    println!("Opening packet capturing library");

    let lib = rawsock::open_best_library().expect("Can't open any library");
    let sockets = SocketSet::new(vec![]);
    let set = RawsockInterfaceSet::new(lib,
        Ipv4Cidr::new(Ipv4Address::new(10, 13, 37, 2), 16),
    ).expect("Could not open any packet capturing library");
    println!("Library opened, version is {}", set.lib_version());
    let (mut opened, errored): (Vec<_>, _) = set.open_all_interface();

    for ErrorWithDesc(err, desc) in errored {
        log::warn!("Err: Interface {:?} err {:?}", desc.name, err);
    }

    for interface in &opened {
        let name = interface.name();
        println!("Interface {} opened, mac: {}, data link: {}", name, interface.mac(), interface.data_link());
    }

    let mut interf = opened.remove(0);

    futures::join!(interf.run());

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
async fn main() {
    env_logger::init().unwrap();
    fuck().await
}
