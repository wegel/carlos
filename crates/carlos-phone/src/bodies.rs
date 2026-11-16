//! Capture checked body lifetimes without exporting cursors or keeping a second window registry.

use std::sync::{Arc, Mutex, Weak};

use carlos_client::BodyToken;
use carlos_proto::ids::EntryId;

use crate::client::State;
use crate::session_calls::session_id;
use crate::{PhoneBatch, PhoneClient, PhoneError, PhoneFrame};

/// An opaque body lifetime tied to exactly one host's client and original checked read.
#[derive(uniffi::Object)]
pub struct PhoneBody {
    owner: Weak<Mutex<State>>,
    pub(crate) token: BodyToken,
}

#[uniffi::export]
impl PhoneBody {
    /// Return the logical session address without revealing native backend identities.
    pub fn session(&self) -> String {
        self.token.session().to_string()
    }

    /// Return the stable row address used to replace this handle after a checked reset.
    pub fn entry(&self) -> String {
        self.token.entry().to_string()
    }

    /// Compare checked lifetimes without exposing their cursors or local generation counters.
    pub fn same_read(&self, other: Arc<PhoneBody>) -> bool {
        Weak::ptr_eq(&self.owner, &other.owner) && self.token == other.token
    }

    /// Keep original source separate from normalized tool output.
    pub fn is_source(&self) -> bool {
        self.token.is_source()
    }
}

impl PhoneBody {
    pub(crate) fn new(owner: &Arc<Mutex<State>>, token: BodyToken) -> Arc<Self> {
        Arc::new(Self {
            owner: Arc::downgrade(owner),
            token,
        })
    }

    pub(crate) fn require_owner(&self, client: &PhoneClient) -> Result<(), PhoneError> {
        if !Weak::ptr_eq(&self.owner, &Arc::downgrade(&client.state)) {
            return Err(PhoneError::InvalidInput {
                message: "This body belongs to a different host client".into(),
            });
        }
        Ok(())
    }
}

#[uniffi::export]
impl PhoneClient {
    /// Open original text at byte zero, optionally retaining a contiguous Markdown document.
    pub fn open_source(
        &self,
        session: String,
        entry: String,
        document: bool,
    ) -> Result<Arc<PhoneBody>, PhoneError> {
        self.open_body(session, entry, true, document)
    }

    /// Open normalized, classified tool pieces independently of their original copy source.
    pub fn open_output(
        &self,
        session: String,
        entry: String,
    ) -> Result<Arc<PhoneBody>, PhoneError> {
        self.open_body(session, entry, false, false)
    }

    /// Issue the next bounded read under the captured lifetime and shared reply checks.
    pub fn read_body(&self, body: Arc<PhoneBody>) -> Result<PhoneFrame, PhoneError> {
        body.require_owner(self)?;
        let mut state = self.lock()?;
        let connection = state.require_connection()?;
        let outgoing = state.client.read_body(connection, &body.token)?;
        state.frame(outgoing)
    }

    /// Subscribe from the accepted cursor; Swift must send the returned frame in order.
    pub fn watch_body(&self, body: Arc<PhoneBody>) -> Result<PhoneFrame, PhoneError> {
        body.require_owner(self)?;
        let mut state = self.lock()?;
        let connection = state.require_connection()?;
        let outgoing = state.client.watch_body(connection, &body.token)?;
        state.frame(outgoing)
    }

    /// Release only this lifetime and return every required unsubscribe frame, even offline.
    pub fn close_body(&self, body: Arc<PhoneBody>) -> Result<PhoneBatch, PhoneError> {
        body.require_owner(self)?;
        let mut state = self.lock()?;
        let effects = state.client.close_body(&body.token);
        state.batch(effects, &self.state)
    }
}

impl PhoneClient {
    fn open_body(
        &self,
        session: String,
        entry: String,
        source: bool,
        document: bool,
    ) -> Result<Arc<PhoneBody>, PhoneError> {
        let session = session_id(&session)?;
        let entry: EntryId = entry.parse().map_err(|_| PhoneError::InvalidInput {
            message: "Invalid entry address".into(),
        })?;
        let mut state = self.lock()?;
        if !state.client.current_session_has_snapshot(&session) {
            return Err(PhoneError::State {
                message: "Wait for the current session snapshot before opening its body".into(),
            });
        }
        let row = state.client.session(&session).and_then(|r| r.entry(&entry));
        let revision = row
            .and_then(|row| {
                if source {
                    row.source
                        .as_ref()
                        .map(|s| s.revision)
                        .or_else(|| row.tool.as_ref().map(|t| t.output_revision))
                } else {
                    row.tool
                        .as_ref()
                        .filter(|t| t.has_output)
                        .map(|t| t.output_revision)
                }
            })
            .ok_or_else(|| PhoneError::State {
                message: "The current row has no separately readable body".into(),
            })?;
        let bodies = state.client.bodies_mut();
        let token = match (source, document) {
            (true, true) => bodies.open_source_document(session, entry, revision)?,
            (true, false) => bodies.open_source(session, entry, revision)?,
            (false, _) => bodies.open_output(session, entry, revision, None)?,
        };
        Ok(PhoneBody::new(&self.state, token))
    }
}
