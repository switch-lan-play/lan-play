use std::fmt::{Display, Error as FmtError, Formatter};

///Kind of library and its version.
#[derive(Debug, Clone)]
pub enum LibraryVersion {
    PCap(String),
    WPCap(String),
    PFRing(String),
}

impl Display for LibraryVersion {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        match self {
            LibraryVersion::PCap(ver) => write!(f, "pcap {}", ver),
            LibraryVersion::WPCap(ver) => write!(f, "wpcap {}", ver),
            LibraryVersion::PFRing(ver) => write!(f, "pfring {}", ver),
        }
    }
}
