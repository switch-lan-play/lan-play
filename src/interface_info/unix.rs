extern crate nix;

use super::{GetAddressError, InterfaceInfo};
use smoltcp::wire::{EthernetAddress};

impl From<nix::Error> for GetAddressError {
    fn from(_: nix::Error) -> GetAddressError {
        GetAddressError::FailedToCallSystem
    }
}

pub fn get_interface_info(name: &str) -> Result<InterfaceInfo, GetAddressError> {
    extern crate nix;
    use nix::{ifaddrs::{getifaddrs}, sys::socket::SockAddr};
    let addrs = getifaddrs()?;
    for ifaddr in addrs {
        if ifaddr.interface_name != name {
            continue;
        }
        if let Some(SockAddr::Link(link)) = ifaddr.address {
            return Ok(InterfaceInfo {
                ethernet_address: EthernetAddress(link.addr()),
                name: name.into(),
                description: None,
            });
        }
    }
    Err(GetAddressError::NotFound)
}
