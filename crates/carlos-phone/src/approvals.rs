//! Bind original live approval cards and only decisions the backend actually offered.

use std::sync::{Arc, Mutex};

use carlos_proto::commands::{ApprovalAnswer, Command};
use carlos_proto::ids::{EntryId, SessionId};
use carlos_proto::session::{Approval, ApprovalInput, Decision};

use crate::client::State;
use crate::session_calls::session_id;
use crate::{PhoneApprovalInput, PhoneClient, PhoneError, PhoneQuestions, PhoneSubmission};

/// A phone cannot manufacture session-wide approval or cancellation support.
pub type PhoneDecision = Decision;
#[uniffi::remote(Enum)]
enum PhoneDecision {
    Once,
    Session,
    Deny,
    Cancel,
}

/// Readonly captions never substitute for the original card's opaque address.
#[derive(uniffi::Record)]
pub struct PhoneApprovalCaption {
    pub kind: String,
    pub title: String,
    pub caller: Option<String>,
    pub details: Vec<String>,
    pub input_error: Option<String>,
}

/// Pending cards can be answered during Starting, before a session accepts prompts.
#[derive(uniffi::Record)]
pub struct PhoneApproval {
    pub id: String,
    pub caption: PhoneApprovalCaption,
    pub choices: Vec<PhoneDecision>,
    pub active: bool,
    pub submitted: bool,
    pub input: Option<PhoneApprovalInput>,
}

#[uniffi::export]
impl PhoneClient {
    /// Copy live card facts in the daemon's order, independently of row history.
    pub fn approvals(&self, session: String) -> Result<Vec<PhoneApproval>, PhoneError> {
        let session = session_id(&session)?;
        let state = self.lock()?;
        Ok(state
            .client
            .session(&session)
            .map(|replica| {
                replica
                    .approvals()
                    .iter()
                    .map(|card| PhoneApproval::new(&self.state, &session, card))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Send one supported decision under its exact current card; structured input stays required.
    pub fn approve(
        &self,
        session: String,
        approval: String,
        decision: PhoneDecision,
    ) -> Result<PhoneSubmission, PhoneError> {
        let session = session_id(&session)?;
        let approval: EntryId = approval.parse().map_err(|_| PhoneError::InvalidInput {
            message: "Invalid approval address".into(),
        })?;
        let mut state = self.lock()?;
        if !state.client.current_session_has_snapshot(&session) {
            return Err(PhoneError::State {
                message: "Wait for this connection's approval facts".into(),
            });
        }
        let card = state
            .client
            .session(&session)
            .and_then(|replica| replica.approvals().iter().find(|card| card.id == approval))
            .ok_or_else(|| PhoneError::State {
                message: "This approval is no longer available".into(),
            })?;
        if !card.active || card.submitted || !card.choices.contains(&decision) {
            return Err(PhoneError::State {
                message: "This card cannot accept that decision".into(),
            });
        }
        if card
            .input
            .as_ref()
            .is_some_and(|input| !matches!(input, ApprovalInput::Browser { .. }))
            && matches!(decision, Decision::Once | Decision::Session)
        {
            return Err(PhoneError::State {
                message: "This request requires structured input".into(),
            });
        }
        state.submit(|operation| Command::Approve {
            session,
            approval,
            operation,
            answer: ApprovalAnswer::Decision { decision },
        })
    }
}

impl PhoneApproval {
    fn new(owner: &Arc<Mutex<State>>, session: &SessionId, card: &Approval) -> Self {
        Self {
            id: card.id.to_string(),
            caption: PhoneApprovalCaption {
                kind: card.kind.clone(),
                title: card.title.clone(),
                caller: card.caller.clone(),
                details: card.details.clone(),
                input_error: card.input_error.clone(),
            },
            choices: card.choices.clone(),
            active: card.active,
            submitted: card.submitted,
            input: card.input.as_ref().map(|input| match input {
                ApprovalInput::Fields { fields } => PhoneApprovalInput::Fields {
                    questions: PhoneQuestions::new(
                        owner,
                        session.clone(),
                        card.id.clone(),
                        fields.clone(),
                    ),
                },
                ApprovalInput::Form { .. } => PhoneApprovalInput::Form,
                ApprovalInput::Browser { url } => PhoneApprovalInput::Browser { url: url.clone() },
                ApprovalInput::Unsupported { reason } => PhoneApprovalInput::Unsupported {
                    reason: reason.clone(),
                },
            }),
        }
    }
}
