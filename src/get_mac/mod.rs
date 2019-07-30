extern crate rawsock;
extern crate smoltcp;
#[cfg(windows)]
mod windows;
use smoltcp::wire::{EthernetAddress};
use std::error::Error as ErrorTrait;
use std::fmt::{Display, Result as FmtResult, Formatter};
use super::rawsock_interface::RawsockInterface;

pub trait GetMac {
    fn get_mac(&self) -> Result<EthernetAddress, MacAddressError>;
}

#[derive(Debug)]
pub enum MacAddressError {
    NotFound,
    LengthMismatch(u32),
}

impl Display for MacAddressError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match *self {
            MacAddressError::NotFound => f.write_str("Mac address not found"),
            MacAddressError::LengthMismatch(l) => write!(f, "LengthMismatch {}", l),
        }
    }
}

#[cfg(target_os="macos")]
impl<'a> GetMac for RawsockInterface<'a> {
    fn get_mac(&self) -> Result<EthernetAddress, MacAddressError> {
        Ok(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]))
    }
}
