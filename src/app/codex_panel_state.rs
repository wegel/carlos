//! Retain bounded descendant/process snapshots separately from the main conversation.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

use serde_json::Value;

pub(super) const MAX_BYTES: usize = 8 * 1024 * 1024;
pub(super) const MAX_AGENTS: usize = 256;
pub(super) const MAX_PROCESSES: usize = 512;

/// Pin destructive controls to an explicit native identity even after its row disappears.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PanelId {
    Agent(String),
    Process {
        thread: String,
        process: String,
        item: Option<String>,
    },
}

/// Keep all panel reads and actions independent of composer and callback drafts.
#[derive(Default)]
pub(super) struct PanelState {
    pub(super) show: bool,
    pub(super) selected: Option<PanelId>,
    pub(super) scroll: usize,
    pub(super) detail: bool,
    pub(super) agents: Vec<Value>,
    pub(super) processes: Vec<Process>,
    pub(super) snapshot: Option<Snapshot>,
    pub(super) activity: Option<Activity>,
    pub(super) stop: Option<Stop>,
    pub(super) outgoing: VecDeque<Request>,
    pub(super) error: Option<String>,
    pub(super) control_error: Option<String>,
    pub(super) notice: String,
    pub(super) refreshed: Option<Instant>,
    pub(super) older: Value,
    pub(super) revision: u64,
    pub(super) reports: HashMap<String, Value>,
    pub(super) older_window: bool,
    pub(super) ownership: VerifiedOwners,
    pub(super) process_scopes: HashMap<String, Option<String>>,
}

#[derive(Default)]
/// Remember only bounded IDs whose parent chains were verified for this exact root and epoch.
pub(super) struct VerifiedOwners {
    root: String,
    epoch: u64,
    ids: HashSet<String>,
}

impl VerifiedOwners {
    /// Install positive facts after snapshot or event parent checks, never from display labels.
    pub(super) fn new(
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
    pub(super) fn contains(&self, root: &str, epoch: u64, id: &str) -> bool {
        self.root == root && self.epoch == epoch && self.ids.contains(id)
    }

    /// Add one child only after its native parent has already been verified.
    pub(super) fn insert(&mut self, root: &str, epoch: u64, id: &str) -> bool {
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
    pub(super) fn ids(&self) -> Vec<String> {
        self.ids.iter().cloned().collect()
    }
}

/// Preserve each terminal's owner without changing its native payload.
pub(super) struct Process {
    pub(super) thread: String,
    pub(super) data: Value,
}

/// Correlate one asynchronous native operation with its exact method and parameters.
#[derive(Clone)]
pub(super) struct Request {
    pub(super) token: u64,
    pub(super) method: String,
    pub(super) params: Value,
}

/// Publish descendant/process pages only after a complete bounded read succeeds.
pub(super) struct Snapshot {
    pub(super) request: Request,
    pub(super) started: Instant,
    pub(super) agents: Vec<Value>,
    pub(super) processes: Vec<Process>,
    pub(super) scopes: VecDeque<String>,
    pub(super) bytes: usize,
    pub(super) cursors: HashSet<String>,
    pub(super) revision: u64,
    pub(super) older: Value,
    pub(super) lineage: HashMap<String, Option<String>>,
    pub(super) checked: HashSet<String>,
    pub(super) errors: Vec<String>,
    pub(super) process_scopes: HashMap<String, Option<String>>,
    pub(super) older_window: bool,
}

/// Keep only the selected child's recent read-only items and bounded live tail.
pub(super) struct Activity {
    pub(super) thread: String,
    pub(super) request: Option<Request>,
    pub(super) started: Instant,
    pub(super) items: Vec<Value>,
    pub(super) cursor: Value,
    pub(super) cursors: HashSet<String>,
    pub(super) bytes: usize,
    pub(super) tail: String,
    pub(super) error: Option<String>,
    pub(super) more: bool,
    pub(super) until: usize,
}

/// A stop acknowledgement never replaces native task/turn/process state.
pub(super) struct Stop {
    pub(super) target: PanelId,
    pub(super) request: Request,
    pub(super) message: String,
}

impl PanelState {
    /// List each native row once, with agents before their background terminals.
    pub(super) fn ids(&self) -> Vec<PanelId> {
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

    /// Keep selection stable; only explicit arrows may replace a vanished selected row.
    pub(super) fn select(&mut self, down: bool) {
        let ids = self.ids();
        let current = self
            .selected
            .as_ref()
            .and_then(|id| ids.iter().position(|row| row == id));
        let next = current.map_or(0, |index| {
            if down {
                (index + 1).min(ids.len().saturating_sub(1))
            } else {
                index.saturating_sub(1)
            }
        });
        self.selected = ids.get(next).cloned();
        self.scroll = 0;
        self.detail = false;
        self.activity = None;
    }

    /// Prevent a vanished or unrelated target from receiving a destructive command.
    pub(super) fn selected_exists(&self) -> bool {
        self.selected
            .as_ref()
            .is_some_and(|id| self.ids().contains(id))
    }
}
