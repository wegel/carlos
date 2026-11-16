//! Read checked transcript windows lazily while Swift retains layout and scroll position.

use carlos_proto::commands::Command;
use carlos_proto::ids::EntryId;

use crate::session_calls::session_id;
use crate::{PhoneClient, PhoneEntry, PhoneError, PhoneFrame, PhoneSession};

/// Small window metadata distinguishes cached and Starting facts from a Ready subscription.
#[derive(uniffi::Record)]
pub struct PhoneTranscript {
    pub info: PhoneSession,
    pub current_snapshot: bool,
    pub ready: bool,
    pub has_older: bool,
    pub has_newer: bool,
    pub count: u64,
}

#[uniffi::export]
impl PhoneClient {
    /// Read window metadata without cloning its transcript.
    pub fn transcript(&self, session: String) -> Result<Option<PhoneTranscript>, PhoneError> {
        let session = session_id(&session)?;
        let state = self.lock()?;
        Ok(state
            .client
            .session(&session)
            .map(|replica| PhoneTranscript {
                info: replica.info().into(),
                current_snapshot: state.client.current_session_has_snapshot(&session),
                ready: state.client.current_session_is_ready(&session),
                has_older: replica.has_older(),
                has_newer: replica.has_newer(),
                count: replica.entry_count() as u64,
            }))
    }

    /// Read a bounded page of chronological stable ids after insertion or replacement.
    pub fn entry_ids(
        &self,
        session: String,
        offset: u32,
        limit: u16,
    ) -> Result<Vec<String>, PhoneError> {
        let session = session_id(&session)?;
        if limit == 0 || limit > 256 {
            return Err(PhoneError::InvalidInput {
                message: "Read one to 256 row addresses at a time".into(),
            });
        }
        let state = self.lock()?;
        let Some(replica) = state.client.session(&session) else {
            return Ok(Vec::new());
        };
        Ok((offset as usize..replica.entry_count())
            .take(limit as usize)
            .filter_map(|index| replica.entry_at(index))
            .map(|entry| entry.id.to_string())
            .collect())
    }

    /// Copy at most 64 explicitly requested rows; streaming does not rescan old row strings.
    pub fn entries(
        &self,
        session: String,
        entries: Vec<String>,
    ) -> Result<Vec<PhoneEntry>, PhoneError> {
        let session = session_id(&session)?;
        if entries.len() > 64 {
            return Err(PhoneError::InvalidInput {
                message: "Read at most 64 visible or changed rows".into(),
            });
        }
        let ids = entries
            .iter()
            .map(|id| id.parse::<EntryId>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PhoneError::InvalidInput {
                message: "Invalid entry address".into(),
            })?;
        let state = self.lock()?;
        let Some(replica) = state.client.session(&session) else {
            return Ok(Vec::new());
        };
        Ok(ids
            .iter()
            .filter_map(|id| replica.entry(id))
            .map(PhoneEntry::from)
            .collect())
    }

    /// Request the actual older window under the shared client's original history fence.
    pub fn older_history(&self, session: String) -> Result<PhoneFrame, PhoneError> {
        let session = session_id(&session)?;
        let mut state = self.lock()?;
        let connection = state.require_connection()?;
        let before = state
            .client
            .session(&session)
            .filter(|replica| replica.has_older())
            .and_then(|replica| replica.entry_at(0))
            .map(|entry| entry.id.clone())
            .ok_or_else(|| PhoneError::State {
                message: "No older history window is available".into(),
            })?;
        let outgoing = state.client.read(
            connection,
            Command::History {
                session,
                before,
                limit: 100,
            },
        )?;
        state.frame(outgoing)
    }
}
