extern crate rawsock;
extern crate smoltcp;
extern crate crossbeam_utils;

mod rawsock_interface;
mod get_addr;

use rawsock_interface::{ErrorWithDesc, RawsockInterfaceSet, RawsockInterface};
use smoltcp::{iface::{EthernetInterfaceBuilder, NeighborCache}, wire::{IpAddress, IpCidr}, socket::SocketSet, time::Instant};
use std::collections::BTreeMap;

fn main() {
    println!("Opening packet capturing library");
    let set = RawsockInterfaceSet::new().expect("Could not open any packet capturing library");
    println!("Library opened, version is {}", set.lib_version());
    let (mut opened, errored): (Vec<_>, _) = set.open_all_interface();

    for ErrorWithDesc(err, desc) in errored {
        println!("Err: Interface {:?} err {:?}", desc.name, err);
    }

    for interface in &opened {
        let name = interface.name();
        println!("Interface {} opened, mac: {}, data link: {}", name, interface.mac(), interface.data_link());
    }

    let device = opened.pop().unwrap();
    let ethernet_addr = device.mac().clone();
    let neighbor_cache = NeighborCache::new(BTreeMap::new());
    let ip_addrs = [
        IpCidr::new(IpAddress::v4(192, 168, 69, 1), 24),
    ];
    let mut iface = EthernetInterfaceBuilder::new(device)
            .ethernet_addr(ethernet_addr)
            .neighbor_cache(neighbor_cache)
            .ip_addrs(ip_addrs)
            .finalize();
    let mut sockets = SocketSet::new(vec![]);
    iface.poll(&mut sockets, Instant::now());
}
