mod error;

#[cfg(windows)]
mod windows;
#[cfg(any(target_os="linux", target_os="macos"))]
mod unix;

#[cfg(windows)]
pub use windows::get_mac;
#[cfg(any(target_os="linux", target_os="macos"))]
pub use unix::get_mac;

pub use error::{GetAddressError};
