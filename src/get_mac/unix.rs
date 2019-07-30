
extern crate nix;
use super::{MacAddressError};
use smoltcp::wire::{EthernetAddress};

impl From<nix::Error> for MacAddressError {
    fn from(_: nix::Error) -> MacAddressError {
        MacAddressError::FailedToCallSystem
    }
}

pub fn get_mac(name: &String) -> Result<EthernetAddress, MacAddressError> {
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
    Err(MacAddressError::NotFound)
}
