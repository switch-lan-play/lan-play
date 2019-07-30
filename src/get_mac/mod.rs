extern crate rawsock;
extern crate smoltcp;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::get_mac;
use smoltcp::wire::{EthernetAddress};
use std::fmt::{Display, Result as FmtResult, Formatter};

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
pub fn get_mac(name: &String) -> Result<EthernetAddress, MacAddressError> {
    Ok(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]))
}
