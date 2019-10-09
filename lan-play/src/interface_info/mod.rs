use smoltcp::wire::EthernetAddress;

mod error;

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

pub use error::{GetAddressError};
