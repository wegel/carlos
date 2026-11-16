//! Bind only the session picker, creation, prompt and reconnect calls used by the app.

use carlos_proto::commands::{Command, RequestId, SendPolicy};
use carlos_proto::ids::SessionId;
use carlos_proto::submissions::ClientOperation;

use crate::client::State;
use crate::operations::key;
use crate::{
    PhoneBackend, PhoneBatch, PhoneClient, PhoneError, PhoneFrame, PhoneOperation, PhoneSession,
    PhoneSubmission,
};

#[uniffi::export]
impl PhoneClient {
    /// Request the live catalogue once; the shared client restores its interest on reconnect.
    pub fn watch_sessions(&self) -> Result<PhoneFrame, PhoneError> {
        let mut state = self.lock()?;
        let connection = state.require_connection()?;
        let outgoing = state.client.read(connection, Command::Watch {})?;
        state.frame(outgoing)
    }

    /// Copy compact picker facts, never transcript bodies.
    pub fn catalog(&self) -> Result<Vec<PhoneSession>, PhoneError> {
        let state = self.lock()?;
        let mut sessions: Vec<_> = state.client.catalog().map(PhoneSession::from).collect();
        sessions.sort_by(|left, right| {
            (&left.host, &left.name, &left.id).cmp(&(&right.host, &right.name, &right.id))
        });
        Ok(sessions)
    }

    /// Select one saved conversation; cached facts remain read-only until refreshed.
    pub fn subscribe(&self, session: String) -> Result<PhoneFrame, PhoneError> {
        let session = session_id(&session)?;
        let mut state = self.lock()?;
        let connection = state.require_connection()?;
        let outgoing = state.client.read(
            connection,
            Command::Subscribe {
                session,
                since: None,
            },
        )?;
        state.frame(outgoing)
    }

    /// Release the previous window without erasing Swift's per-row display preferences.
    pub fn unsubscribe(&self, session: String) -> Result<PhoneFrame, PhoneError> {
        let session = session_id(&session)?;
        let mut state = self.lock()?;
        let connection = state.require_connection()?;
        let outgoing = state
            .client
            .read(connection, Command::Unsubscribe { session })?;
        state.frame(outgoing)
    }

    /// Create exactly one session with an explicit working directory and selected backend.
    pub fn create_session(
        &self,
        backend: PhoneBackend,
        cwd: String,
    ) -> Result<PhoneSubmission, PhoneError> {
        if cwd.trim().is_empty() {
            return Err(PhoneError::InvalidInput {
                message: "Enter a working directory on the host".into(),
            });
        }
        self.lock()?.submit(|operation| Command::Create {
            operation,
            backend,
            cwd,
            name: None,
            tool_labels: None,
            launch: Default::default(),
        })
    }

    /// Retain original text before writing; running turns receive the explicit queue policy.
    pub fn send_prompt(
        &self,
        session: String,
        text: String,
    ) -> Result<PhoneSubmission, PhoneError> {
        let session = session_id(&session)?;
        if text.trim().is_empty() {
            return Err(PhoneError::InvalidInput {
                message: "Enter a prompt first".into(),
            });
        }
        let mut state = self.lock()?;
        if !state.client.current_session_is_ready(&session) {
            return Err(PhoneError::State {
                message: "Wait for the current session to become ready".into(),
            });
        }
        state.submit(|operation| Command::Send {
            session,
            text,
            operation,
            policy: SendPolicy::Queue,
        })
    }

    /// Read this device's exact private results, including failed submissions whose writes never began.
    pub fn operations(&self) -> Result<Vec<PhoneOperation>, PhoneError> {
        Ok(self
            .lock()?
            .client
            .operations()
            .iter()
            .map(PhoneOperation::from)
            .collect())
    }

    /// Fetch one changed private result without repeatedly copying all retained prompt strings.
    pub fn operation(&self, operation: String) -> Result<Option<PhoneOperation>, PhoneError> {
        Ok(self
            .lock()?
            .client
            .operations()
            .iter()
            .find(|record| key(&record.operation) == operation)
            .map(PhoneOperation::from))
    }

    /// Release a handled result only after Swift retained any text the user still needs.
    pub fn forget_operation(&self, operation: String) -> Result<(), PhoneError> {
        let mut state = self.lock()?;
        let address = state
            .client
            .operations()
            .iter()
            .find(|record| key(&record.operation) == operation)
            .map(|record| record.operation.clone());
        if let Some(address) = address {
            state.client.forget_operation(&address);
        }
        Ok(())
    }

    /// Fence an original failed write; a late old socket cannot invalidate its replacement.
    pub fn write_failed(
        &self,
        generation: u64,
        request_id: i64,
        possibly_written: bool,
    ) -> Result<PhoneBatch, PhoneError> {
        let mut state = self.lock()?;
        let Some(connection) = state.connection(generation) else {
            return Ok(PhoneBatch::ignored(state.ready()));
        };
        let effects = state.client.write_failed(
            connection,
            RequestId::Integer(request_id),
            "Connection lost while sending; submitted text retained",
            possibly_written,
        );
        state.retire();
        state.batch(effects, &self.state)
    }
}

impl State {
    pub(crate) fn submit(
        &mut self,
        build: impl FnOnce(ClientOperation) -> Command,
    ) -> Result<PhoneSubmission, PhoneError> {
        let connection = self.require_connection()?;
        let mut address = None;
        let outgoing = self.client.submit(connection, |operation| {
            address = Some(operation.clone());
            build(operation)
        })?;
        let record = address
            .as_ref()
            .and_then(|operation| self.client.operations().get(operation))
            .ok_or_else(|| PhoneError::Internal {
                message: "submitted command lost its private record".into(),
            })?;
        Ok(PhoneSubmission {
            frame: self.frame(outgoing)?,
            operation: PhoneOperation::from(record),
        })
    }
}

pub(crate) fn session_id(value: &str) -> Result<SessionId, PhoneError> {
    value.parse().map_err(|_| PhoneError::InvalidInput {
        message: "Invalid session address".into(),
    })
}
