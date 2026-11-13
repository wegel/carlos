//! Retain bounded descendant/process snapshots separately from the main conversation.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

use serde_json::Value;

pub(in crate::session) const MAX_BYTES: usize = 8 * 1024 * 1024;
pub(in crate::session) const MAX_AGENTS: usize = 256;
pub(in crate::session) const MAX_PROCESSES: usize = 512;

/// Pin destructive controls to an explicit native identity even after its row disappears.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PanelId {
    Agent(String),
    Process {
        thread: String,
        process: String,
        item: Option<String>,
    },
}

/// Keep all panel reads and actions independent of composer and callback drafts.
#[derive(Debug, Default)]
pub struct PanelState {
    pub(in crate::session) agents: Vec<Value>,
    pub(in crate::session) processes: Vec<Process>,
    pub(in crate::session) snapshot: Option<Snapshot>,
    pub(in crate::session) activity: Option<Activity>,
    pub(in crate::session) stop: Option<Stop>,
    pub(in crate::session) outgoing: VecDeque<Request>,
    pub(in crate::session) error: Option<String>,
    pub(in crate::session) control_error: Option<String>,
    pub(in crate::session) notice: String,
    pub(in crate::session) refreshed: Option<Instant>,
    pub(in crate::session) older: Value,
    pub(in crate::session) revision: u64,
    pub(in crate::session) reports: HashMap<String, Value>,
    pub(in crate::session) older_window: bool,
    pub(in crate::session) ownership: VerifiedOwners,
    pub(in crate::session) process_scopes: HashMap<String, Option<String>>,
}

#[derive(Debug, Default)]
/// Remember only bounded IDs whose parent chains were verified for this exact root and epoch.
pub(crate) struct VerifiedOwners {
    root: String,
    epoch: u64,
    ids: HashSet<String>,
}

impl VerifiedOwners {
    /// Install positive facts after snapshot or event parent checks, never from display labels.
    pub fn new(
        root: &str,
        epoch: u64,
        ids: impl IntoIterator<Item = String>,
    ) -> Result<Self, String> {
        let mut verified = Self {
            root: root.into(),
            epoch,
            ids: HashSet::new(),
        };
        for id in ids {
            if id.is_empty() || id.len() > 4096 || verified.ids.len() >= 512 {
                return Err("Verified task ownership exceeds its 512-ID bound".into());
            }
            verified.ids.insert(id);
        }
        Ok(verified)
    }

    /// Keep redraws constant-time and make branch changes invalidate old facts immediately.
    pub fn contains(&self, root: &str, epoch: u64, id: &str) -> bool {
        self.root == root && self.epoch == epoch && self.ids.contains(id)
    }

    /// Add one child only after its native parent has already been verified.
    pub(in crate::session) fn insert(&mut self, root: &str, epoch: u64, id: &str) -> bool {
        if self.root != root || self.epoch != epoch {
            *self = Self {
                root: root.into(),
                epoch,
                ids: HashSet::new(),
            };
        }
        if self.ids.len() >= 512 && !self.ids.contains(id) {
            return false;
        }
        self.ids.insert(id.into());
        true
    }

    /// Carry the off-thread snapshot's already-verified facts through its private handoff.
    pub fn ids(&self) -> Vec<String> {
        self.ids.iter().cloned().collect()
    }
}

/// Preserve each terminal's owner without changing its native payload.
#[derive(Debug)]
pub struct Process {
    pub(in crate::session) thread: String,
    pub(in crate::session) data: Value,
}

/// Correlate one asynchronous native operation with its exact method and parameters.
#[derive(Clone, Debug)]
pub struct Request {
    pub(in crate::session) token: u64,
    pub(in crate::session) method: String,
    pub(in crate::session) params: Value,
}

/// Publish descendant/process pages only after a complete bounded read succeeds.
#[derive(Debug)]
pub struct Snapshot {
    pub(in crate::session) request: Request,
    pub(in crate::session) started: Instant,
    pub(in crate::session) agents: Vec<Value>,
    pub(in crate::session) processes: Vec<Process>,
    pub(in crate::session) scopes: VecDeque<String>,
    pub(in crate::session) bytes: usize,
    pub(in crate::session) cursors: HashSet<String>,
    pub(in crate::session) revision: u64,
    pub(in crate::session) older: Value,
    pub(in crate::session) lineage: HashMap<String, Option<String>>,
    pub(in crate::session) checked: HashSet<String>,
    pub(in crate::session) errors: Vec<String>,
    pub(in crate::session) process_scopes: HashMap<String, Option<String>>,
    pub(in crate::session) older_window: bool,
}

/// Keep only the selected child's recent read-only items and bounded live tail.
#[derive(Debug)]
pub struct Activity {
    pub(in crate::session) thread: String,
    pub(in crate::session) request: Option<Request>,
    pub(in crate::session) started: Instant,
    pub(in crate::session) items: Vec<Value>,
    pub(in crate::session) cursor: Value,
    pub(in crate::session) cursors: HashSet<String>,
    pub(in crate::session) bytes: usize,
    pub(in crate::session) tail: String,
    pub(in crate::session) error: Option<String>,
    pub(in crate::session) more: bool,
    pub(in crate::session) until: usize,
}

/// A stop acknowledgement never replaces native task/turn/process state.
#[derive(Debug)]
pub struct Stop {
    pub(in crate::session) target: PanelId,
    pub(in crate::session) request: Request,
    pub(in crate::session) message: String,
}

impl PanelState {
    /// List each native row once, with agents before their background terminals.
    pub fn ids(&self) -> Vec<PanelId> {
        self.agents
            .iter()
            .filter_map(|agent| agent["id"].as_str())
            .map(|id| PanelId::Agent(id.into()))
            .chain(self.processes.iter().filter_map(|process| {
                Some(PanelId::Process {
                    thread: process.thread.clone(),
                    process: process.data["processId"].as_str()?.into(),
                    item: process.data["itemId"].as_str().map(str::to_string),
                })
            }))
            .collect()
    }

    /// Test an explicitly supplied target against the current bounded native rows.
    pub fn contains_target(&self, selected: Option<&PanelId>) -> bool {
        selected.is_some_and(|id| self.ids().contains(id))
    }
}

impl PanelState {
    /// Borrow or read the native panel agents.
    pub fn agents(&self) -> &[Value] {
        &self.agents
    }
    /// Borrow or read the native panel processes.
    pub fn processes(&self) -> &[Process] {
        &self.processes
    }
    /// Borrow or read the native panel snapshot.
    #[cfg(any(test, feature = "test-support"))]
    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshot.as_ref()
    }
    /// Borrow or read the native panel activity.
    pub fn activity(&self) -> Option<&Activity> {
        self.activity.as_ref()
    }
    /// Borrow or read the native panel stop.
    pub fn stop(&self) -> Option<&Stop> {
        self.stop.as_ref()
    }
    /// Borrow or read the native panel error.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    /// Borrow or read the native panel control error.
    pub fn control_error(&self) -> Option<&str> {
        self.control_error.as_deref()
    }
    /// Borrow or read the native panel notice.
    pub fn notice(&self) -> &str {
        &self.notice
    }
    /// Borrow or read the native panel older.
    #[cfg(any(test, feature = "test-support"))]
    pub fn older(&self) -> &Value {
        &self.older
    }
    /// Borrow or read the native panel reports.
    pub fn reports(&self) -> &HashMap<String, Value> {
        &self.reports
    }
    /// Borrow or read the native panel older window.
    #[cfg(any(test, feature = "test-support"))]
    pub fn older_window(&self) -> bool {
        self.older_window
    }
}

impl Process {
    /// Construct a native process record with its exact owner.
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(thread: String, data: Value) -> Self {
        Self { thread, data }
    }
    /// Borrow the verified native owner.
    pub fn thread(&self) -> &str {
        &self.thread
    }
    /// Borrow the original native process fields.
    pub fn data(&self) -> &Value {
        &self.data
    }
}

impl Activity {
    /// Borrow the exact native child whose items were read.
    pub fn thread(&self) -> &str {
        &self.thread
    }
    /// Borrow a native read failure without changing retained items.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    /// Borrow the bounded native item cache.
    pub fn items(&self) -> &[Value] {
        &self.items
    }
    /// Check whether a native page remains pending.
    pub fn reading(&self) -> bool {
        self.request.is_some()
    }
    /// Check whether the native cursor reports older items.
    pub fn more(&self) -> bool {
        self.more
    }
    /// Borrow the bounded live output since the last item snapshot.
    pub fn tail(&self) -> &str {
        &self.tail
    }
}
impl Stop {
    /// Borrow native stop progress independently of panel focus.
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[cfg(any(test, feature = "test-support"))]
impl PanelState {
    /// Report whether the fixture has pending native panel requests.
    pub fn has_outgoing(&self) -> bool {
        !self.outgoing.is_empty()
    }
}
#[cfg(any(test, feature = "test-support"))]
impl Stop {
    /// Borrow the fixture stop request associated with this task.
    pub fn request(&self) -> &Request {
        &self.request
    }
}
