extern crate rawsock;
extern crate smoltcp;
extern crate crossbeam_utils;
extern crate env_logger;

mod rawsock_interface;
mod get_addr;
mod duplex;

use rawsock_interface::{ErrorWithDesc, RawsockInterfaceSet};
use smoltcp::{
    socket::{SocketSet, TcpSocket, TcpSocketBuffer},
};
use rawsock::{traits::Library, Error as RawsockError};

static mut RAWSOCK_LIB: Result<Box<dyn Library>, RawsockError> = Err(RawsockError::NoPathsProvided);

fn main() {
    env_logger::init();
    println!("Opening packet capturing library");
    let shit = rawsock::pcap::Library::open_default_paths();

    let lib = match unsafe {
        RAWSOCK_LIB = rawsock::open_best_library();
        &RAWSOCK_LIB
    } {
        Ok(lib) => lib,
        Err(err) => panic!(err)
    };
    let set = RawsockInterfaceSet::new(lib).expect("Could not open any packet capturing library");
    println!("Library opened, version is {}", set.lib_version());
    let (mut opened, errored): (Vec<_>, _) = set.open_all_interface();

    for ErrorWithDesc(err, desc) in errored {
        println!("Err: Interface {:?} err {:?}", desc.name, err);
    }

    for interface in &opened {
        let name = interface.name();
        println!("Interface {} opened, mac: {}, data link: {}", name, interface.mac(), interface.data_link());
    }
    set.start(opened);

    // let mut iface = EthernetInterfaceBuilder::new(device)
    //         .ethernet_addr(ethernet_addr)
    //         .neighbor_cache(neighbor_cache)
    //         .ip_addrs(ip_addrs)
    //         .finalize();

    let tcp2_socket = TcpSocket::new(
        TcpSocketBuffer::new(vec![0; 64]),
        TcpSocketBuffer::new(vec![0; 128])
    );

    let mut sockets = SocketSet::new(vec![]);
    let tcp2_handle = sockets.add(tcp2_socket);
    let mut tcp2_active = false;

    // loop {
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

    //         if socket.can_send() {
    //             println!("yeah!!!");
    //             socket.send_slice(b"yeah");
    //         }

    //         println!("  state: {}", socket.state());
    //     }
    //     match iface.poll(&mut sockets, Instant::now()) {
    //         Err(smoltcp::Error::Unrecognized) => continue,
    //         Err(err) => {
    //             println!("poll err {}", err);
    //         },
    //         Ok(_) => ()
    //     }
    // }
}
