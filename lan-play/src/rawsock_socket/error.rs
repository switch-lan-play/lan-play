use crate::interface_info::GetAddressError;
use rawsock::InterfaceDescription;
use futures::channel::mpsc;
use futures::channel::oneshot;

#[derive(Debug)]
pub enum Error {
    RawsockErr(rawsock::Error),
    WrongDataLink(rawsock::DataLink),
    GetAddr(GetAddressError),
    Mpsc(mpsc::SendError),
    OneshotCancel(oneshot::Canceled),
    SmolTcpErr(smoltcp::Error),
    Other(&'static str),
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct ErrorWithDesc (pub Error, pub InterfaceDescription);

impl From<smoltcp::Error> for Error {
    fn from(err: smoltcp::Error) -> Self {
        Error::SmolTcpErr(err)
    }
}

impl From<rawsock::Error> for Error {
    fn from(err: rawsock::Error) -> Self {
        Error::RawsockErr(err)
    }
}

impl From<GetAddressError> for Error {
    fn from(err: GetAddressError) -> Self {
        Error::GetAddr(err)
    }
}

impl From<mpsc::SendError> for Error {
    fn from(err: mpsc::SendError) -> Self {
        Error::Mpsc(err)
    }
}

impl From<oneshot::Canceled> for Error {
    fn from(err: oneshot::Canceled) -> Self {
        Error::OneshotCancel(err)
    }
}

