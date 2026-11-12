//! Own tool-label requests, cancellation, and the current entry bindings.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use super::entry_id::EntryId;
use super::state::SessionState;
#[cfg(test)]
use super::transcript::Transcript;
use crate::summaries::{Request, SummaryService};

#[derive(Debug)]
/// Keep one live request beside its literal and generated labels.
pub(in crate::session) struct Binding {
    pub(in crate::session) request: Request,
    pub(in crate::session) original: String,
    pub(in crate::session) label: Option<String>,
}

/// Own accepted label preferences and pending work without an editor draft.
pub(in crate::session) struct SummaryState {
    pub(in crate::session) enabled: bool,
    pub(in crate::session) cwd: PathBuf,
    pub(in crate::session) bindings: HashMap<EntryId, Binding>,
    pub(in crate::session) pending: VecDeque<EntryId>,
    pub(in crate::session) service: Option<SummaryService>,
    pub(in crate::session) replaying: bool,
    pub(in crate::session) revision: u64,
}

impl SummaryState {
    /// Create empty label facts from an explicit saved preference.
    pub(in crate::session) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            cwd: PathBuf::new(),
            bindings: HashMap::new(),
            pending: VecDeque::new(),
            service: None,
            replaying: false,
            revision: 0,
        }
    }
}

impl SummaryState {
    /// Cancel pending model jobs before discarding their entry bindings.
    pub(in crate::session) fn clear(&mut self) {
        if let Some(service) = &self.service {
            service.cancel_pending();
        }
        self.bindings.clear();
        self.pending.clear();
    }

    #[cfg(test)]
    /// Retain bindings only for entries that still exist after a rewind.
    pub(in crate::session) fn retain_items(&mut self, items: &Transcript) {
        self.bindings.retain(|id, _| items.entry(*id).is_some());
        self.pending.retain(|id| self.bindings.contains_key(id));
    }

    /// Try pending requests in order without blocking on a full worker queue.
    pub(in crate::session) fn flush(&mut self) {
        if !self.enabled {
            return;
        }
        let Some(service) = &self.service else {
            return;
        };
        while let Some(id) = self.pending.front() {
            if let Some(binding) = self.bindings.get(id) {
                if !service.try_request(binding.request.clone()) {
                    break;
                }
            }
            self.pending.pop_front();
        }
    }

    /// Read a generated label only while the accepted preference enables it.
    pub(in crate::session) fn label(&self, id: EntryId) -> Option<String> {
        self.enabled
            .then(|| self.bindings.get(&id)?.label.clone())
            .flatten()
    }
}

#[cfg(test)]
impl std::fmt::Debug for SummaryState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SummaryState")
            .field("enabled", &self.enabled)
            .field("cwd", &self.cwd)
            .field("bindings", &self.bindings)
            .field("pending", &self.pending)
            .field("replaying", &self.replaying)
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

impl SessionState {
    /// Read the accepted tool-label preference.
    pub(crate) fn summaries_enabled(&self) -> bool {
        self.summaries.enabled
    }

    /// Read the source workspace used by tool labels.
    pub(crate) fn summary_cwd(&self) -> &Path {
        &self.summaries.cwd
    }

    /// Set the source workspace supplied by the backend launch caller.
    pub(crate) fn set_summary_cwd(&mut self, cwd: PathBuf) {
        self.summaries.cwd = cwd;
    }

    #[cfg(test)]
    /// Cancel pending model work and discard current bindings without stopping the service.
    pub(crate) fn clear_summaries(&mut self) {
        self.summaries.clear();
    }

    /// Mark history reads without allowing replayed events to start model work.
    pub(crate) fn set_summary_replaying(&mut self, replaying: bool) -> bool {
        std::mem::replace(&mut self.summaries.replaying, replaying)
    }

    #[cfg(test)]
    pub(crate) fn retain_summary_items(&mut self) {
        self.summaries.retain_items(&self.messages);
    }

    #[cfg(test)]
    pub(crate) fn set_summary_enabled_for_test(&mut self, enabled: bool) {
        self.summaries.enabled = enabled;
    }

    #[cfg(test)]
    pub(crate) fn summary_binding_for_test(&self, id: EntryId) -> Option<String> {
        Some(self.summaries.bindings.get(&id)?.request.binding.clone())
    }

    #[cfg(test)]
    pub(crate) fn assert_summary_invariants(&self) {
        for id in self.summaries.bindings.keys() {
            assert!(self.messages.entry(*id).is_some());
        }
        for id in &self.summaries.pending {
            assert!(self.summaries.bindings.contains_key(id));
        }
    }
}
