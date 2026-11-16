//! Return explicit socket writes and checked greeting results across the Swift boundary.

use std::sync::Arc;

use carlos_client::Outbound;
use carlos_proto::commands::RequestId;

use crate::{PhoneError, PhoneSessionChange};

/// One ordered socket write; its payload can contain credentials and must never be logged.
#[derive(uniffi::Record)]
pub struct PhoneFrame {
    pub generation: u64,
    pub request_id: i64,
    pub payload: String,
}

impl PhoneFrame {
    pub(crate) fn from_outbound(generation: u64, outgoing: Outbound) -> Result<Self, PhoneError> {
        let RequestId::Integer(request_id) = outgoing.id() else {
            return Err(PhoneError::Internal {
                message: "unexpected client request address".into(),
            });
        };
        let payload =
            String::from_utf8(outgoing.into_bytes()).map_err(|_| PhoneError::Internal {
                message: "client encoded a non-UTF-8 request".into(),
            })?;
        Ok(Self {
            generation,
            request_id,
            payload,
        })
    }
}

/// A peer rejected one request; this never grants permission to resend a mutation.
#[derive(uniffi::Record)]
pub struct PhoneFailure {
    pub code: i32,
    pub message: String,
}

/// Consume every outgoing frame and failure before reflecting readiness in the UI.
#[derive(uniffi::Record)]
pub struct PhoneBatch {
    pub outgoing: Vec<PhoneFrame>,
    pub ready: bool,
    pub failures: Vec<PhoneFailure>,
    pub catalog_changed: bool,
    pub sessions: Vec<PhoneSessionChange>,
    pub operations: Vec<String>,
    pub bodies: Vec<Arc<crate::PhoneBody>>,
}

impl PhoneBatch {
    pub(crate) fn ignored(ready: bool) -> Self {
        Self {
            outgoing: Vec::new(),
            ready,
            failures: Vec::new(),
            catalog_changed: false,
            sessions: Vec::new(),
            operations: Vec::new(),
            bodies: Vec::new(),
        }
    }
}
