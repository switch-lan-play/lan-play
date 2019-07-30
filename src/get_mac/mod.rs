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
}

impl Display for MacAddressError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match *self {
            MacAddressError::NotFound => f.write_str("Mac address not found"),
        }
    }
}

#[cfg(target_os="macos")]
pub fn get_mac(name: &String) -> Result<EthernetAddress, MacAddressError> {
    Ok(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]))
}
