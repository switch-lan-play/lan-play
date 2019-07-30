extern crate winapi;
use super::{GetMac, MacAddressError};
use crate::rawsock_interface::RawsockInterface;
use smoltcp::wire::{EthernetAddress};

#[cfg(windows)]
impl<'a> GetMac for RawsockInterface<'a> {
    fn get_mac(&self) -> Result<EthernetAddress, MacAddressError> {
        let name = &self.desc.name;
        Ok(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]))
    }
}
