use crate::get_addr::GetAddressError;
use rawsock::InterfaceDescription;

#[derive(Debug)]
pub enum Error {
    RawsockErr(rawsock::Error),
    WrongDataLink(rawsock::DataLink),
    GetAddr(GetAddressError),
    Other(&'static str),
}
#[derive(Debug)]
pub struct ErrorWithDesc (pub Error, pub InterfaceDescription);

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

