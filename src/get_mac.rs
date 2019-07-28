extern crate rawsock;
extern crate smoltcp;
use smoltcp::wire::{EthernetAddress};

pub trait GetMac {
    fn get_mac(&self) -> Option<EthernetAddress>;
}

// #[cfg(windows)]
impl<'a> GetMac for (rawsock::traits::Interface<'a> + 'a) {
    fn get_mac(&self) -> Option<EthernetAddress> {
        Some(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]))
    }
}
