//! Cache compact task facts after native task changes without rescanning task bodies on transcript deltas.

use std::time::Instant;

use carlos_proto::session::Backend;
use carlos_proto::task_facts::{TaskCounts, TaskFacts};

use super::codex::task_badge;
use super::state::SessionState;
use super::task_digest::hash_text;
use super::task_rows::{clipped, TaskRef};

/// Dirty flags describe owner writes, never a client's selected row or polling clock.
#[derive(Debug)]
pub(super) struct TaskPublication {
    stamp: Option<MetadataStamp>,
    facts: Option<TaskFacts>,
    approval_revision: u64,
    metadata_dirty: bool,
    overview_dirty: bool,
    #[cfg(test)]
    metadata_rebuilds: u64,
    #[cfg(test)]
    overview_rebuilds: u64,
}

#[derive(Debug, PartialEq, Eq)]
struct MetadataStamp {
    scope: blake3::Hash,
    enabled: bool,
    revision: u64,
    refreshed: Option<Instant>,
    reading: Option<u64>,
    runs: blake3::Hash,
}

impl Default for TaskPublication {
    fn default() -> Self {
        Self {
            stamp: None,
            facts: None,
            approval_revision: 0,
            metadata_dirty: true,
            overview_dirty: true,
            #[cfg(test)]
            metadata_rebuilds: 0,
            #[cfg(test)]
            overview_rebuilds: 0,
        }
    }
}

impl SessionState {
    /// Mark an actual native metadata write without touching the transcript journal.
    pub(in crate::session) fn task_metadata_changed(&mut self) {
        self.task_publication.metadata_dirty = true;
        self.task_publication.overview_dirty = true;
    }

    /// Main activity, accepted-input state and quota writes invalidate only the small overview.
    pub(in crate::session) fn task_overview_changed(&mut self) {
        self.task_publication.overview_dirty = true;
    }

    pub(super) fn refresh_task_facts(&mut self) -> bool {
        let stamp = self.task_metadata_stamp();
        let metadata_changed = self.task_publication.metadata_dirty
            || self.task_publication.stamp.as_ref() != Some(&stamp);
        let overview_changed = metadata_changed || self.task_publication.overview_dirty;
        let counts_changed = self.events.backend == Backend::Codex
            && self.task_publication.approval_revision != self.approval.revision;
        if !overview_changed && !counts_changed {
            return false;
        }
        #[cfg(test)]
        {
            self.task_publication.metadata_rebuilds += u64::from(metadata_changed);
            self.task_publication.overview_rebuilds += u64::from(overview_changed);
        }
        let previous = self.task_publication.facts.take();
        let facts = self.build_task_facts(previous.as_ref(), metadata_changed, overview_changed);
        let changed = facts != previous;
        self.task_publication.facts = facts;
        self.task_publication.stamp = Some(stamp);
        self.task_publication.approval_revision = self.approval.revision;
        self.task_publication.metadata_dirty = false;
        self.task_publication.overview_dirty = false;
        changed
    }

    pub(super) fn export_tasks(&self) -> Option<TaskFacts> {
        self.task_publication.facts.clone()
    }

    fn build_task_facts(
        &self,
        previous: Option<&TaskFacts>,
        metadata_changed: bool,
        overview_changed: bool,
    ) -> Option<TaskFacts> {
        if self.events.backend == Backend::Grok
            || self.events.backend == Backend::Codex && !self.codex_scope().enabled()
        {
            return None;
        }
        let (revision, mut counts) = match (metadata_changed, previous) {
            (false, Some(previous)) => (previous.revision.clone(), previous.counts.clone()),
            _ => {
                let rows = self.task_rows();
                (self.task_revision(&rows), self.task_counts(&rows))
            }
        };
        counts.active = self.task_active_count();
        let overview_revision = match (overview_changed, previous) {
            (false, Some(previous)) => previous.overview_revision.clone(),
            _ => self.task_overview_revision(),
        };
        Some(TaskFacts {
            revision,
            overview_revision,
            known: self.task_known(),
            reading: self.codex_panel.snapshot.is_some(),
            closed: self.events.backend_closed,
            counts,
            native_older_available: !self.codex_panel.older.is_null(),
            read_error: self
                .codex_panel
                .error
                .as_deref()
                .map(|error| clipped(error, 2048).into()),
            notice: clipped(&self.codex_panel.notice, 2048).into(),
        })
    }

    fn task_counts(&self, rows: &[TaskRef<'_>]) -> TaskCounts {
        let mut counts = TaskCounts::default();
        for row in rows {
            match row {
                TaskRef::Agent { .. } => counts.agents += 1,
                TaskRef::Process(_) => counts.processes += 1,
                TaskRef::Claude { .. } => counts.tasks += 1,
            }
        }
        counts
    }

    fn task_active_count(&self) -> u32 {
        match self.events.backend {
            Backend::Claude => self.claude.tasks.active_count() as u32,
            Backend::Codex => task_badge::count(self, self.backend_context().panel) as u32,
            Backend::Grok => 0,
        }
    }

    fn task_metadata_stamp(&self) -> MetadataStamp {
        MetadataStamp {
            scope: self.task_scope().finalize(),
            enabled: match self.events.backend {
                Backend::Codex => self.codex_scope().enabled(),
                Backend::Claude => self.claude.enabled,
                Backend::Grok => false,
            },
            revision: self.codex_panel.revision,
            refreshed: self.codex_panel.refreshed,
            reading: self
                .codex_panel
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.request.token),
            runs: self.task_run_stamp(),
        }
    }

    fn task_run_stamp(&self) -> blake3::Hash {
        let mut hash = blake3::Hasher::new();
        let mut reads: Vec<_> = self.codex_panel.runs.iter().collect();
        reads.sort_by(|left, right| left.0.cmp(right.0));
        for (thread, read) in reads {
            hash_text(&mut hash, thread);
            hash_text(&mut hash, read.id.as_str());
            hash.update(&[
                u8::from(read.request.is_some()),
                u8::from(read.error.is_some()),
            ]);
            if let Some(turn) = &read.turn {
                hash_text(&mut hash, &turn.id);
                hash_text(&mut hash, &turn.status);
            }
        }
        hash.finalize()
    }
}

#[cfg(test)]
#[path = "task_publication_tests.rs"]
mod tests;
