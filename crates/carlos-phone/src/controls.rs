//! Capture the original turn or session for explicit Stop, reopen and Ralph controls.

use carlos_proto::commands::{Command, SessionOperation};

use crate::session_calls::session_id;
use crate::{PhoneClient, PhoneError, PhoneSubmission};

#[uniffi::export]
impl PhoneClient {
    /// Rename the selected native session without changing its prompt or starting a turn.
    pub fn rename_session(
        &self,
        session: String,
        name: String,
    ) -> Result<PhoneSubmission, PhoneError> {
        let session = session_id(&session)?;
        if name.len() > 4096 || name.chars().any(char::is_control) {
            return Err(PhoneError::InvalidInput {
                message: "Use a single-line name of at most 4 KiB".into(),
            });
        }
        let mut state = self.lock()?;
        state.require_ready_session(&session)?;
        state.submit(|operation| Command::Rename {
            session,
            operation,
            name,
        })
    }

    /// Interrupt the actual current turn without guessing its result or denying unrelated cards.
    pub fn interrupt(&self, session: String) -> Result<PhoneSubmission, PhoneError> {
        let session = session_id(&session)?;
        let mut state = self.lock()?;
        if !state.client.current_session_has_snapshot(&session) {
            return Err(PhoneError::State {
                message: "Wait for this connection's current turn".into(),
            });
        }
        let turn = state
            .client
            .session(&session)
            .and_then(|replica| replica.info().current_turn.clone())
            .ok_or_else(|| PhoneError::State {
                message: "No current turn can be interrupted".into(),
            })?;
        state.submit(|operation| Command::Interrupt {
            session,
            operation,
            turn,
        })
    }

    /// Reopen an explicitly chosen closed session while keeping its logical conversation identity.
    pub fn reopen(&self, session: String) -> Result<PhoneSubmission, PhoneError> {
        let session = session_id(&session)?;
        self.lock()?
            .submit(|operation| Command::Reopen(SessionOperation { session, operation }))
    }

    /// Request Ralph's actual state; a queued operation never becomes an immediate shared fact.
    pub fn set_ralph(&self, session: String, on: bool) -> Result<PhoneSubmission, PhoneError> {
        let session = session_id(&session)?;
        let mut state = self.lock()?;
        if !state.client.current_session_is_ready(&session) {
            return Err(PhoneError::State {
                message: "Wait for the session to become ready".into(),
            });
        }
        state.submit(|operation| Command::SetRalph {
            session,
            operation,
            on,
            options: None,
        })
    }
}
