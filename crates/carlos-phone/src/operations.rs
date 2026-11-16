//! Keep exact private submissions and their observed outcomes available to the phone.

use carlos_client::LocalOperation;
use carlos_proto::submissions::{ClientOperation, OperationState};

use crate::PhoneFrame;

/// An acknowledgement is distinct from queued work and an unknown write outcome.
#[derive(uniffi::Enum)]
pub enum PhoneOperationState {
    Pending,
    Queued,
    Accepted { notice: Option<String> },
    Rejected { message: String },
    Uncertain { message: String },
}

/// Only this device can see its submitted text or recover it into its composer.
#[derive(uniffi::Record)]
pub struct PhoneOperation {
    pub key: String,
    pub session: Option<String>,
    pub text: Option<String>,
    pub needs_echo: bool,
    pub state: PhoneOperationState,
}

/// The caller retains the operation before attempting its single ordered socket write.
#[derive(uniffi::Record)]
pub struct PhoneSubmission {
    pub frame: PhoneFrame,
    pub operation: PhoneOperation,
}

pub(crate) fn key(operation: &ClientOperation) -> String {
    format!("{}:{}", operation.client, operation.id.get())
}

impl From<&LocalOperation> for PhoneOperation {
    fn from(operation: &LocalOperation) -> Self {
        let state = match &operation.state {
            OperationState::Pending => PhoneOperationState::Pending,
            OperationState::Queued => PhoneOperationState::Queued,
            OperationState::Accepted { notice, .. } => PhoneOperationState::Accepted {
                notice: notice.clone(),
            },
            OperationState::Rejected { message, .. } => PhoneOperationState::Rejected {
                message: message.clone(),
            },
            OperationState::Uncertain { message, .. } => PhoneOperationState::Uncertain {
                message: message.clone(),
            },
        };
        Self {
            key: key(&operation.operation),
            session: operation.session.as_ref().map(ToString::to_string),
            text: operation.text().map(str::to_owned),
            needs_echo: operation.needs_echo(),
            state,
        }
    }
}
