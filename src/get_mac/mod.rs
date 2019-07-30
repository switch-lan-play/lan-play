#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::get_mac;

#[cfg(any(target_os="linux", target_os="macos"))]
mod unix;
#[cfg(any(target_os="linux", target_os="macos"))]
pub use unix::get_mac;

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
