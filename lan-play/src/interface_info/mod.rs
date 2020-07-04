#![allow(dead_code)]

use smoltcp::wire::EthernetAddress;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Mac address not found")]
    NotFound,
    #[error("Failed to call system")]
    FailedToCallSystem,
}

pub struct InterfaceInfo {
    pub ethernet_address: EthernetAddress,
    pub name: String,
    pub description: Option<String>,
}

impl Default for InterfaceInfo {
    fn default() -> Self {
        InterfaceInfo {
            ethernet_address: EthernetAddress::BROADCAST,
            name: "".to_string(),
            description: None,
        }
    }
}

cfg_if! {
    if #[cfg(windows)] {
        mod windows;
        pub use windows::get_interface_info;
    } else if #[cfg(unix)] {
        mod unix;
        pub use unix::get_interface_info;
    }
}
