use crate::interface_info;
use rawsock::InterfaceDescription;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("RawsockErr {0:?}")]
    RawsockErr(#[from] rawsock::Error),
    #[error("WrongDataLink {0:?}")]
    WrongDataLink(rawsock::DataLink),
    #[error("GetAddr {0:?}")]
    GetAddr(#[from] interface_info::Error),
    #[error("Other {0}")]
    Other(&'static str),
}
#[derive(Debug)]
pub struct ErrorWithDesc(pub Error, pub InterfaceDescription);
