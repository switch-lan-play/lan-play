extern crate rawsock;
extern crate smoltcp;
#[cfg(windows)] extern crate winapi;
use smoltcp::wire::{EthernetAddress};
use std::error::Error as ErrorTrait;
use std::fmt::{Display, Result as FmtResult, Formatter};
use super::rawsock_interface::RawsockInterface;

pub trait GetMac {
    fn get_mac(&self) -> Result<EthernetAddress, MacAddressError>;
}

#[derive(Debug)]
pub enum MacAddressError {
    DllError
}

impl ErrorTrait for MacAddressError {
    fn description(&self) -> &str {
        match *self {
            MacAddressError::DllError => "error"
        }
    }
}

impl Display for MacAddressError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match *self {
            MacAddressError::DllError => f.write_str("DllError")
        }
    }
}

#[cfg(windows)]
impl<'a> GetMac for RawsockInterface<'a> {
    fn get_mac(&self) -> Result<EthernetAddress, MacAddressError> {
        let name = self.desc.name;
        Ok(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]))
    }
}

#[cfg(target_os="macos")]
impl<'a> GetMac for RawsockInterface<'a> {
    fn get_mac(&self) -> Result<EthernetAddress, MacAddressError> {
        Ok(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]))
    }
}
