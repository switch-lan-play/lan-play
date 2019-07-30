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
    FailedToCallSystem,
}

impl Display for MacAddressError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match *self {
            MacAddressError::NotFound => f.write_str("Mac address not found"),
            MacAddressError::FailedToCallSystem => f.write_str("Failed to call system"),
        }
    }
}

#[cfg(any(target_os="linux", target_os="macos"))]
pub fn get_mac(name: &String) -> Result<EthernetAddress, MacAddressError> {
    extern crate nix;
    use nix::{ifaddrs::{getifaddrs}, sys::socket::SockAddr};
    let addrs = getifaddrs()?;
    for ifaddr in addrs {
        if ifaddr.interface_name != name {
            continue;
        }
        if let SockAddr::Link(link) = address {
            let bytes = link.addr();
        }
    }
    Ok(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]))
}
