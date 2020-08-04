mod raw_udp;
mod reactor;
mod socket;
mod socketset;
mod device;
mod channel_sink;
mod ethernet;

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
pub use socket::{SocketHandle, TcpListener, TcpSocket, UdpSocket, SendHalf, RecvHalf};
pub use socketset::BufferSize;
use socketset::SocketSet;
use std::collections::BTreeMap;
use device::ChannelDevice;
use std::sync::Arc;
use channel_sink::SinkSender;

pub type Ethernet = SmoltcpEthernetInterface<'static, 'static, 'static, ChannelDevice>;

pub struct Net {
    reactor: Arc<NetReactor>,
}

impl Net {
    pub fn new(
        ethernet_addr: EthernetAddress,
        ip_addrs: Vec<IpCidr>,
        gateway_ip: Ipv4Address,
        interf: RawsockInterface,
        intercepter: IntercepterBuilder,
        mtu: usize,
        buffer_size: BufferSize,
    ) -> Net {
        let (_running, tx, rx) = interf.start(
            intercepter
        );
        let device = ChannelDevice::new(SinkSender(tx), rx, mtu);
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

        let reactor = NetReactor::new(buffer_size);
        let r = reactor.clone();
        tokio::spawn(async move {
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
