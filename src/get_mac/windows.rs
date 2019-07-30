extern crate winapi;
use crate::get_mac::GetMac;

#[cfg(windows)]
pub impl<'a> GetMac for RawsockInterface<'a> {
    fn get_mac(&self) -> Result<EthernetAddress, MacAddressError> {
        let name = &self.desc.name;
        Ok(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]))
    }
}
