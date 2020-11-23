use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("RawsockErr {0:?}")]
    RawsockError(#[from] rawsock::Error),
    #[error("IoError {0:?}")]
    IoError(#[from] std::io::Error),
    #[error("NoInterface")]
    NoInterface,
    #[error("Timed out")]
    Timedout(#[from] tokio::time::error::Elapsed),
    #[error("Smoltcp error {0:?}")]
    Smoltcp(smoltcp::Error),
    #[error("Bad Packet")]
    BadPacket,
}
pub type Result<T> = std::result::Result<T, Error>;

impl From<smoltcp::Error> for Error {
    fn from(e: smoltcp::Error) -> Self {
        Error::Smoltcp(e)
    }
}