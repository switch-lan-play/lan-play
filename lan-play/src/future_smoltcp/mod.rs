mod peekable_receiver;
mod raw_udp;
mod reactor;
mod socket;
mod socketset;
mod device;

use crate::interface::{RawsockInterface, IntercepterBuilder};
pub use raw_udp::OwnedUdp;
use reactor::NetReactor;
use smoltcp::{
    iface::{
        EthernetInterface as SmoltcpEthernetInterface, EthernetInterfaceBuilder, NeighborCache,
        Routes,
    },
    wire::{EthernetAddress, IpCidr, Ipv4Address},
};
pub use socket::{SocketHandle, TcpListener, TcpSocket, UdpSocket};
use socketset::SocketSet;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use device::FutureDevice;

pub type Ethernet = SmoltcpEthernetInterface<'static, 'static, 'static, FutureDevice>;

pub struct Net {
    reactor: NetReactor,
}

impl Net {
    pub fn new(
        ethernet_addr: EthernetAddress,
        ip_addrs: Vec<IpCidr>,
        gateway_ip: Ipv4Address,
        interf: RawsockInterface,
        intercepter: IntercepterBuilder,
    ) -> Net {
        let (_running, tx, rx) = interf.start(
            intercepter
        );
        let device = FutureDevice::new(tx, rx);
        let neighbor_cache = NeighborCache::new(BTreeMap::new());
        let mut routes = Routes::new(BTreeMap::new());
        routes.add_default_ipv4_route(gateway_ip).unwrap();

        let ethernet = EthernetInterfaceBuilder::new(device)
            .ethernet_addr(ethernet_addr)
            .ip_addrs(ip_addrs)
            .neighbor_cache(neighbor_cache)
            .any_ip(true)
            .routes(routes)
            .finalize();

        let socket_set = Arc::new(Mutex::new(SocketSet::new()));
        let reactor = NetReactor::new(socket_set);
        let r = reactor.clone();
        crate::rt::spawn(async move {
            r.run(ethernet).await
        });

        Net {
            reactor,
        }
    }
    pub async fn tcp_listener(&self) -> TcpListener {
        TcpListener::new(self.reactor.clone()).await
    }
    pub async fn udp_socket(&self) -> UdpSocket {
        UdpSocket::new(self.reactor.clone()).await
    }
}
