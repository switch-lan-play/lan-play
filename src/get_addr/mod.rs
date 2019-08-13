mod error;

cfg_if! {
    if #[cfg(windows)] {
        mod windows;
        pub use windows::get_mac;
    } else if #[cfg(unix)] {
        mod unix;
        pub use unix::get_mac;
    }
}

pub use error::{GetAddressError};
