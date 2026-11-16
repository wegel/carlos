//! Preserve actionable failure classes without asking Swift to interpret error strings.

use std::fmt;

use carlos_client::ClientError;

/// A caller retains private input and distinguishes capacity from a fatal protocol error.
#[derive(Debug, uniffi::Error)]
pub enum PhoneError {
    /// Caller-supplied text or an address is invalid; no request was sent.
    InvalidInput { message: String },
    /// A bounded cache or request allowance is full.
    Capacity { message: String },
    /// The socket generation or client state cannot perform this call.
    State { message: String },
    /// The peer sent invalid facts; close this transport before attempting recovery.
    InvalidMessage { message: String },
    /// The binding cannot safely access its client; retain private input and replace it.
    Internal { message: String },
}

impl From<ClientError> for PhoneError {
    fn from(error: ClientError) -> Self {
        let message = error.to_string();
        match error {
            ClientError::Capacity(_) => Self::Capacity { message },
            ClientError::InvalidConnection => Self::State { message },
            _ => Self::InvalidMessage { message },
        }
    }
}

impl fmt::Display for PhoneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (Self::InvalidInput { message }
        | Self::Capacity { message }
        | Self::State { message }
        | Self::InvalidMessage { message }
        | Self::Internal { message }) = self;
        formatter.write_str(message)
    }
}

impl std::error::Error for PhoneError {}
