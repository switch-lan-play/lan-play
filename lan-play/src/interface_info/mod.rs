use smoltcp::wire::EthernetAddress;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Mac address not found")]
    NotFound,
    #[error("Failed to call system")]
    FailedToCallSystem
}


pub struct InterfaceInfo {
    pub ethernet_address: EthernetAddress,
    pub name: String,
    pub description: Option<String>,
}

impl InterfaceInfo {
    pub fn new(name: &str) -> InterfaceInfo {
        InterfaceInfo {
            ethernet_address: EthernetAddress::BROADCAST,
            name: name.into(),
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
