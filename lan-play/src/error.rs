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
    Timedout(#[from] crate::rt::Elapsed)
}
pub type Result<T> = std::result::Result<T, Error>;
