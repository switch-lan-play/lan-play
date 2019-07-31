use std::fmt::{Display, Result as FmtResult, Formatter};

#[derive(Debug)]
pub enum GetAddressError {
    NotFound,
    FailedToCallSystem,
}

impl Display for GetAddressError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match *self {
            GetAddressError::NotFound => f.write_str("Mac address not found"),
            GetAddressError::FailedToCallSystem => f.write_str("Failed to call system"),
        }
    }
}
