extern crate rawsock;
extern crate smoltcp;
extern crate crossbeam_utils;

mod rawsock_interface;
mod get_addr;

use rawsock_interface::{ErrorWithDesc, RawsockInterfaceSet};
use smoltcp::{
    iface::{EthernetInterfaceBuilder, NeighborCache},
    wire::{IpAddress, IpCidr},
    socket::{SocketSet, TcpSocket, TcpSocketBuffer},
    time::Instant};
use std::collections::BTreeMap;
use rawsock::{traits::Library, Error as RawsockError};

fn main() {
    println!("Opening packet capturing library");
    static rawsockLib: Result<Box<dyn Library>, RawsockError> = rawsock::open_best_library();
    let lib = match rawsockLib {
        Ok(lib) => lib,
        Err(err) => panic!(err)
    };
    let set = RawsockInterfaceSet::new(&lib).expect("Could not open any packet capturing library");
    println!("Library opened, version is {}", set.lib_version());
    let (mut opened, errored): (Vec<_>, _) = set.open_all_interface();

    for ErrorWithDesc(err, desc) in errored {
        println!("Err: Interface {:?} err {:?}", desc.name, err);
    }

    for interface in &opened {
        let name = interface.name();
        println!("Interface {} opened, mac: {}, data link: {}", name, interface.mac(), interface.data_link());
    }

    let device = opened.remove(0);
    let ethernet_addr = device.mac().clone();
    let neighbor_cache = NeighborCache::new(BTreeMap::new());
    let ip_addrs = [
        IpCidr::new(IpAddress::v4(192, 168, 233, 2), 24),
    ];
    let mut iface = EthernetInterfaceBuilder::new(device)
            .ethernet_addr(ethernet_addr)
            .neighbor_cache(neighbor_cache)
            .ip_addrs(ip_addrs)
            .finalize();

    let tcp2_socket = TcpSocket::new(
        TcpSocketBuffer::new(vec![0; 65535]),
        TcpSocketBuffer::new(vec![0; 65535])
    );

    let mut sockets = SocketSet::new(vec![]);
    let tcp2_handle = sockets.add(tcp2_socket);

    {
        let mut socket = sockets.get::<TcpSocket>(tcp2_handle);
        socket.listen(1234).expect("can not listen to 1234");
    }
    loop {
        match iface.poll(&mut sockets, Instant::now()) {
            Err(smoltcp::Error::Unrecognized) => continue,
            Err(err) => {
                println!("poll {}", err);
            },
            Ok(_) => ()
        }

        {
            let mut socket = sockets.get::<TcpSocket>(tcp2_handle);
            if socket.can_send() {
                println!("yeah!!!");
            }
        }
    }
}
