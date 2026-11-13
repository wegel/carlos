//! Publish bounded, numbered changes while borrowing the facts they describe.

use carlos_proto::events::{Event, SessionEvent};
use carlos_proto::ids::{Sequence, SessionId};
use carlos_proto::session::{Approval, Backend, SessionInfo};
use uuid::Uuid;

use super::entry_id::EntryId;
use super::event_rows::is_tool;
use super::state::SessionState;
use super::transcript_journal::{Journal, MAX_PENDING};

/// One cheap record for an in-process client; ids are never row offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Added(EntryId),
    Replaced(EntryId),
    Text { entry: EntryId, from: usize },
    Facts(EntryId),
    Approval(usize),
    ApprovalResolved(EntryId),
    Info,
    Reset { dirty_from: Option<EntryId> },
}

/// A record and its session-local cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberedChange {
    pub seq: Sequence,
    pub change: Change,
}

#[derive(Debug)]
pub(super) struct EventState {
    pub(super) id: SessionId,
    pub(super) backend: Backend,
    pub(super) host: String,
    pub(super) seq: Sequence,
    pub(super) info: Option<SessionInfo>,
    pub(super) approvals: Vec<Approval>,
    pub(super) approval_revision: u64,
    pub(super) turn_started_at: Option<i64>,
}

impl EventState {
    pub(super) fn new() -> Self {
        Self {
            id: Uuid::new_v4().simple().to_string().parse().expect("uuid"),
            backend: Backend::Codex,
            host: "local".to_owned(),
            seq: Sequence::default(),
            info: None,
            approvals: Vec::new(),
            approval_revision: 0,
            turn_started_at: None,
        }
    }
}

/// Immutable facts sealed at the last cursor; encode before the next session write.
pub struct Published<'a> {
    pub(super) session: &'a SessionState,
    records: Vec<NumberedChange>,
}

impl SessionState {
    /// Bind the actual backend and host, independently of compatibility controls.
    pub fn set_client_identity(&mut self, backend: Backend, host: String) {
        self.events.backend = backend;
        self.events.host = host;
    }

    /// Borrow the logical session address, preserved across native forks and reloads.
    pub fn session_id(&self) -> &SessionId {
        &self.events.id
    }

    /// Seal pending writes once; holding the result prevents mutation during encoding.
    pub fn publish(&mut self) -> Published<'_> {
        let count = self.messages.len();
        let journal = std::mem::replace(
            &mut self.messages.journal,
            Journal {
                published: count,
                reset: false,
                ..Journal::default()
            },
        );
        let mut changes = self.row_changes(journal);
        self.publish_approvals(&mut changes);
        if self.info_changed() {
            self.events.info = Some(self.export_info());
            changes.push(Change::Info);
        }
        // A reset already describes the entire current state. Never follow it with
        // deltas referring to facts included by the same reset snapshot.
        if let Some(reset) = changes
            .iter()
            .find(|c| matches!(c, Change::Reset { .. }))
            .copied()
        {
            changes.clear();
            changes.push(reset);
        } else if changes.len() > MAX_PENDING * 2 + 1 {
            changes.clear();
            changes.push(Change::Reset { dirty_from: None });
        }
        let records = changes
            .into_iter()
            .map(|change| {
                self.events.seq = self
                    .events
                    .seq
                    .checked_next()
                    .expect("session sequence exhausted");
                NumberedChange {
                    seq: self.events.seq,
                    change,
                }
            })
            .collect();
        Published {
            session: self,
            records,
        }
    }

    fn row_changes(&self, journal: Journal) -> Vec<Change> {
        let messages = &self.messages;
        if journal.reset
            || journal.overflow
            || messages.len().saturating_sub(journal.published) > MAX_PENDING
        {
            let dirty_from = (!journal.reset)
                .then(|| {
                    messages
                        .get(journal.earliest.min(journal.published))
                        .map(|m| m.id)
                })
                .flatten();
            return vec![Change::Reset { dirty_from }];
        }
        let mut changes = Vec::new();
        for row in journal.rows {
            if row.replace {
                changes.push(Change::Replaced(row.id));
            } else {
                if let Some(from) = row.delta {
                    changes.push(Change::Text {
                        entry: row.id,
                        from,
                    });
                }
                if row.facts {
                    changes.push(Change::Facts(row.id));
                }
            }
        }
        changes.extend(
            messages[journal.published..]
                .iter()
                .map(|m| Change::Added(m.id)),
        );
        changes
    }

    fn publish_approvals(&mut self, changes: &mut Vec<Change>) {
        if self.events.approval_revision == self.approval.revision {
            return;
        }
        let current = self.export_approvals();
        for old in &self.events.approvals {
            if !current.iter().any(|card| card.id == old.id) {
                changes.push(Change::ApprovalResolved(EntryId::from_wire(&old.id)));
            }
        }
        for (index, card) in current.iter().enumerate() {
            if !self.events.approvals.contains(card) {
                changes.push(Change::Approval(index));
            }
        }
        self.events.approvals = current;
        self.events.approval_revision = self.approval.revision;
    }
}

impl Published<'_> {
    /// Inspect the same numbered changes without allocating or serializing row text.
    pub fn records(&self) -> &[NumberedChange] {
        &self.records
    }

    /// Read the sealed cursor even when this publication contained no changes.
    pub fn sequence(&self) -> Sequence {
        self.session.events.seq
    }

    /// Borrow the logical session address.
    pub fn session_id(&self) -> &SessionId {
        self.session.session_id()
    }

    /// Resolve a stable row id for an in-process cache while the session is borrowed.
    pub fn position(&self, id: EntryId) -> Option<usize> {
        self.session.messages.position(id)
    }

    /// Read the contiguous row count without materializing a wire snapshot.
    pub fn entry_count(&self) -> usize {
        self.session.messages.len()
    }

    /// Materialize exactly one sealed event for a transport or headless client.
    pub fn to_wire(&self, record: &NumberedChange) -> Option<SessionEvent> {
        if !self.records.contains(record) {
            return None;
        }
        let session = self.session;
        let event = match record.change {
            Change::Added(id) => Event::EntryAdded {
                entry: session.export_entry(session.messages.entry(id)?),
            },
            Change::Replaced(id) => Event::EntryUpdated {
                entry: session.export_entry(session.messages.entry(id)?),
            },
            Change::Text { entry, from } => {
                let message = session.messages.entry(entry)?;
                if is_tool(message) {
                    Event::EntryFactsChanged {
                        entry: entry.to_wire(),
                        facts: session.export_entry_facts(message),
                    }
                } else {
                    Event::TextAppended {
                        entry: entry.to_wire(),
                        text: message.text.get(from..)?.to_owned(),
                    }
                }
            }
            Change::Facts(id) => Event::EntryFactsChanged {
                entry: id.to_wire(),
                facts: session.export_entry_facts(session.messages.entry(id)?),
            },
            Change::Approval(index) => Event::ApprovalRequested {
                approval: session.events.approvals.get(index)?.clone(),
            },
            Change::ApprovalResolved(id) => Event::ApprovalResolved {
                approval: id.to_wire(),
            },
            Change::Info => Event::InfoChanged {
                info: session.events.info.clone()?,
            },
            Change::Reset { .. } => Event::Reset {
                snapshot: self.snapshot(100),
            },
        };
        Some(SessionEvent {
            session: session.session_id().clone(),
            seq: record.seq,
            event,
        })
    }
}
