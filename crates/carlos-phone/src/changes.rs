//! Carry changed addresses across the boundary without copying growing transcripts.

use carlos_client::SessionChanges;

/// A replaced or bounded row window needs its chronological ids read again.
#[derive(uniffi::Record)]
pub struct PhoneWindowChange {
    pub reset: bool,
    pub removed_front: u64,
    pub removed_back: u64,
}

/// Swift fetches only changed or visible rows and preserves its own scroll/draft state.
#[derive(uniffi::Record)]
pub struct PhoneSessionChange {
    pub session: String,
    pub entries: Vec<String>,
    pub bodies: Vec<String>,
    pub info: bool,
    pub approvals: bool,
    pub window: PhoneWindowChange,
}

impl From<SessionChanges> for PhoneSessionChange {
    fn from(changed: SessionChanges) -> Self {
        let changes = changed.changes;
        Self {
            session: changed.session.to_string(),
            entries: changes
                .entries
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
            bodies: changes
                .bodies
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
            info: changes.info,
            approvals: changes.approvals,
            window: PhoneWindowChange {
                reset: changes.reset,
                removed_front: changes.removed_front as u64,
                removed_back: changes.removed_back as u64,
            },
        }
    }
}
