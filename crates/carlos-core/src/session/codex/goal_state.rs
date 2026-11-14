//! Own native goals and pending backend requests without answer editors.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;

use serde_json::Value;

/// Only known non-driving state may permit Carlos to enable a separate automatic driver.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GoalKnowledge {
    #[default]
    Unknown,
    Known,
    Unavailable,
}

#[derive(Debug, Default)]
/// Own the accepted native goal and its pending reads and changes.
pub struct GoalState {
    pub(in crate::session) knowledge: GoalKnowledge,
    pub(in crate::session) goal: Value,
    pub(in crate::session) revision: u64,
    pub(in crate::session) read: Option<GoalRequest>,
    pub(in crate::session) mutation: Option<GoalRequest>,
    pub(in crate::session) error: Option<String>,
    pub(in crate::session) read_error: Option<String>,
    pub(in crate::session) notice: String,
    pub(in crate::session) last_read: Option<Instant>,
    pub(in crate::session) direct_input: Option<bool>,
}
/// Match each acknowledgement to its exact scope and the facts known before dispatch.
#[derive(Debug)]
pub(crate) struct GoalRequest {
    pub(in crate::session) live: Arc<AtomicBool>,
    pub(in crate::session) operation: Option<carlos_proto::submissions::ClientOperation>,
    pub(in crate::session) token: u64,
    pub(in crate::session) method: String,
    pub(in crate::session) params: Value,
    pub(in crate::session) revision: u64,
    pub(in crate::session) may_activate: bool,
}

impl Drop for GoalRequest {
    fn drop(&mut self) {
        self.live.store(false, Ordering::SeqCst);
    }
}

impl GoalState {
    /// Read the native goal knowledge.
    pub fn knowledge(&self) -> GoalKnowledge {
        self.knowledge
    }
    /// Borrow the latest native objective, status and counters.
    pub fn goal(&self) -> &Value {
        &self.goal
    }
    /// Check whether a native goal read remains pending.
    pub fn reading(&self) -> bool {
        self.read.is_some()
    }
    /// Check whether a submitted goal change remains pending.
    pub fn pending(&self) -> bool {
        self.mutation.is_some()
    }
    /// Read the native goal error.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    /// Read the native goal read error.
    pub fn read_error(&self) -> Option<&str> {
        self.read_error.as_deref()
    }
    /// Read the native goal notice.
    pub fn notice(&self) -> &str {
        &self.notice
    }
    /// Read the native goal last read.
    #[cfg(any(test, feature = "test-support"))]
    pub fn last_read(&self) -> Option<Instant> {
        self.last_read
    }
}
