extern crate nix;

use super::{GetAddressError};
use smoltcp::wire::{EthernetAddress};

impl From<nix::Error> for GetAddressError {
    fn from(_: nix::Error) -> GetAddressError {
        GetAddressError::FailedToCallSystem
    }
}

pub fn get_mac(name: &String) -> Result<EthernetAddress, GetAddressError> {
    extern crate nix;
    use nix::{ifaddrs::{getifaddrs}, sys::socket::SockAddr};
    let addrs = getifaddrs()?;
    for ifaddr in addrs {
        if ifaddr.interface_name != *name {
            continue;
        }
        if let Some(SockAddr::Link(link)) = ifaddr.address {
            return Ok(EthernetAddress(link.addr()));
        }
    }
    Err(GetAddressError::NotFound)
}
