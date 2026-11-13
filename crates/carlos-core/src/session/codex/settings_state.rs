//! Own accepted native settings and submitted update requests without editor drafts.

use serde_json::Value;

/// Retain requested side effects with accepted work until the native request starts.
#[derive(Debug, Default)]
pub(in crate::session) struct SettingsFlags {
    pub(in crate::session) persist: bool,
    pub(in crate::session) announce: bool,
}

/// Keep acknowledged native choices separate from pending editor requests.
#[derive(Debug, Default)]
pub struct SettingsState {
    pub(in crate::session) flags: SettingsFlags,
    pub(in crate::session) future: Value,
    pub(in crate::session) revision: u64,
    pub(in crate::session) pending: Option<Change>,
    pub(in crate::session) deferred: Option<Value>,
    pub(in crate::session) active_before: Option<(String, String)>,
    pub(in crate::session) errors: Value,
    pub(in crate::session) legacy_overrides: bool,
}

/// Settle future and current-turn writes independently for one requested choice.
#[derive(Debug)]
pub(in crate::session) struct Change {
    pub(in crate::session) token: u64,
    pub(in crate::session) desired: Value,
    pub(in crate::session) revision: u64,
    pub(in crate::session) thread: Option<String>,
    pub(in crate::session) turn: Option<String>,
    pub(in crate::session) turn_id: Option<String>,
    pub(in crate::session) persist: bool,
    pub(in crate::session) announce: bool,
}

impl SettingsState {
    /// Read the accepted native settings future.
    pub fn future(&self) -> &Value {
        &self.future
    }
    /// Read the accepted native settings revision.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Read the accepted native settings pending.
    #[cfg(any(test, feature = "test-support"))]
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
    /// Read the accepted native settings deferred.
    #[cfg(any(test, feature = "test-support"))]
    pub fn deferred(&self) -> Option<&Value> {
        self.deferred.as_ref()
    }
    /// Read the accepted native settings errors.
    #[cfg(any(test, feature = "test-support"))]
    pub fn errors(&self) -> &Value {
        &self.errors
    }
    /// Read the accepted native settings legacy overrides.
    pub fn legacy_overrides(&self) -> bool {
        self.legacy_overrides
    }
}
