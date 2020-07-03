#[derive(Debug)]
pub enum Error {
    RawsockError(rawsock::Error),
    NoInterface,
}
pub type Result<T> = std::result::Result<T, Error>;

impl From<rawsock::Error> for Error {
    fn from(e: rawsock::Error) -> Error {
        Error::RawsockError(e)
    }
}
