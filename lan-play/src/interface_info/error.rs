use std::fmt::{Display, Result as FmtResult, Formatter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GetAddressError {
    #[error("Mac address not found")]
    NotFound,
    #[error("Failed to call system")]
    FailedToCallSystem
}
