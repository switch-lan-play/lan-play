use smoltcp::{
    iface::{
        EthernetInterface as SmoltcpEthernetInterface, EthernetInterfaceBuilder, NeighborCache,
        Routes,
    },
    wire::{EthernetAddress, IpCidr, Ipv4Address},
};
use super::{ChannelDevice, SocketSet};
use futures::future::poll_fn;
use smoltcp::{socket::TcpState, time::{Duration, Instant}};

pub type Ethernet = SmoltcpEthernetInterface<'static, 'static, 'static, ChannelDevice>;

struct FutureEthernet {
    inner: Ethernet,
}

impl FutureEthernet {
    async fn poll_ethernet(&self, ethernet: &mut Ethernet, set: &mut SocketSet) -> bool {
        todo!()
        // let now = Instant::now();
        
        // let r = poll_fn(|cx| {
        //     ethernet.device_mut().waker = Some(cx.waker().clone());
        //     match ethernet.poll(set.as_set_mut(), now) {
        //         Ok(true) => true,
        //         // readiness not changed
        //         Ok(false) | Err(smoltcp::Error::Dropped) => false,
        //         Err(e) => {
        //             log::error!("poll error {:?}", e);
        //             false
        //         }
        //     }
        // }).await;
        // ethernet.device_mut().waker = None;
        // r
    }
}
