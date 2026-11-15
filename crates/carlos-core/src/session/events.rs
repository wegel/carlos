//! Publish bounded, numbered changes while borrowing the facts they describe.

use std::collections::{HashMap, HashSet};

use carlos_proto::events::{Event, SessionEvent};
use carlos_proto::ids::{Sequence, SessionId};
use carlos_proto::session::{Approval, Backend, Decision, SessionInfo};
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
    Text {
        entry: EntryId,
        from: usize,
    },
    Facts(EntryId),
    Approval(EntryId),
    ApprovalResolved {
        approval: EntryId,
        decision: Option<Decision>,
    },
    Info,
    Goal,
    Queue,
    Tasks,
    Settings,
    Reset {
        dirty_from: Option<EntryId>,
    },
}

/// A record and its session-local cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberedChange {
    pub seq: Sequence,
    pub change: Change,
}

#[derive(Debug)]
pub(super) struct EventState {
    pub(super) settings: Option<carlos_proto::settings::SettingsFacts>,
    pub(super) queue_fingerprint: Option<blake3::Hash>,
    pub(super) goal_fingerprint: Option<blake3::Hash>,
    pub(super) operations: super::submission_identity::OperationEchoes,
    pub(super) current_turn: Option<super::control_liveness::LiveTurn>,
    pub(super) backend_closed: bool,
    pub(super) launch: Option<carlos_proto::session::Availability>,
    pub(super) ralph_restore_notice: bool,
    pub(super) launch_name: Option<String>,
    pub(super) id: SessionId,
    pub(super) backend: Backend,
    pub(super) host: String,
    pub(super) seq: Sequence,
    pub(super) info: Option<SessionInfo>,
    pub(super) approvals: Vec<Approval>,
    pub(super) approval_revision: u64,
    pub(super) decisions: Vec<(EntryId, Decision)>,
    pub(super) decision_overflow: bool,
    pub(super) turn_started_at: Option<i64>,
}

impl EventState {
    pub(super) fn new() -> Self {
        Self {
            settings: None,
            queue_fingerprint: None,
            goal_fingerprint: None,
            operations: Default::default(),
            current_turn: None,
            backend_closed: false,
            launch: None,
            ralph_restore_notice: false,
            launch_name: None,
            id: Uuid::new_v4().simple().to_string().parse().expect("uuid"),
            backend: Backend::Codex,
            host: "local".to_owned(),
            seq: Sequence::default(),
            info: None,
            approvals: Vec::new(),
            approval_revision: 0,
            decisions: Vec::new(),
            decision_overflow: false,
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

    pub(in crate::session) fn sealed(&self) -> Published<'_> {
        debug_assert!(
            self.events.info.is_some(),
            "seal owner writes before serving reads"
        );
        Published {
            session: self,
            records: Vec::new(),
        }
    }

    /// Seal pending writes once; holding the result prevents mutation during encoding.
    pub fn publish(&mut self) -> Published<'_> {
        self.refresh_wire_turn();
        let count = self.messages.len();
        let journal = std::mem::replace(
            &mut self.messages.journal,
            Journal {
                published: count,
                reset: false,
                ..Journal::default()
            },
        );
        if journal.reset {
            self.retain_entry_operations();
        }
        let mut changes = self.row_changes(journal);
        self.publish_controls(&mut changes);
        if std::mem::take(&mut self.events.decision_overflow) {
            changes.clear();
            changes.push(Change::Reset { dirty_from: None });
        }
        self.events.decisions.clear();
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

    fn publish_controls(&mut self, changes: &mut Vec<Change>) {
        self.publish_approvals(changes);
        self.publish_goal(changes);
        self.publish_queue(changes);
        self.publish_settings(changes);
        if self.refresh_task_facts() {
            changes.push(Change::Tasks);
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
        let live: HashSet<_> = current.iter().map(|card| &card.id).collect();
        for old in &self.events.approvals {
            if !live.contains(&old.id) {
                let approval = EntryId::from_wire(&old.id);
                let decision = self
                    .events
                    .decisions
                    .iter()
                    .find(|(id, _)| *id == approval)
                    .map(|(_, choice)| *choice);
                changes.push(Change::ApprovalResolved { approval, decision });
            }
        }
        let old: HashMap<_, _> = self
            .events
            .approvals
            .iter()
            .enumerate()
            .map(|(index, card)| {
                (
                    &card.id,
                    (
                        card,
                        self.events.approvals.get(index + 1).map(|next| &next.id),
                    ),
                )
            })
            .collect();
        // Publish anchors first, including moves whose card facts did not change.
        for (index, card) in current.iter().enumerate().rev() {
            let next = current.get(index + 1).map(|next| &next.id);
            if old.get(&card.id) != Some(&(card, next)) {
                changes.push(Change::Approval(EntryId::from_wire(&card.id)));
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
        let offset = record
            .seq
            .get()
            .checked_sub(self.records.first()?.seq.get())?;
        let index = usize::try_from(offset).ok()?;
        if self.records.get(index) != Some(record) {
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
            Change::Text { entry, from } => self.text_event(entry, from)?,
            Change::Facts(id) => Event::EntryFactsChanged {
                entry: id.to_wire(),
                facts: session.export_entry_facts(session.messages.entry(id)?),
            },
            Change::Approval(id) => self.approval_event(id)?,
            Change::Goal => Event::GoalChanged {
                goal: self.session.export_goal(),
            },
            Change::Queue => Event::QueueChanged {
                queue: self.session.export_queue(),
            },
            Change::Tasks => Event::TasksChanged {
                tasks: self.session.export_tasks(),
            },
            Change::Settings => Event::SettingsChanged {
                settings: self.session.events.settings.clone(),
            },
            Change::ApprovalResolved { approval, decision } => Event::ApprovalResolved {
                approval: approval.to_wire(),
                decision,
            },
            Change::Info => Event::InfoChanged {
                info: session.events.info.clone()?,
            },
            Change::Reset { .. } => Event::Reset {
                snapshot: Box::new(self.snapshot(100)),
            },
        };
        Some(SessionEvent {
            session: session.session_id().clone(),
            seq: record.seq,
            event,
        })
    }

    fn text_event(&self, entry: EntryId, from: usize) -> Option<Event> {
        let session = self.session;
        let message = session.messages.entry(entry)?;
        Some(if is_tool(message) {
            Event::OutputChanged {
                entry: entry.to_wire(),
                revision: carlos_proto::output::OutputRevision::new(
                    session.messages.output_revision(entry),
                ),
                has_output: !message.text.is_empty(),
            }
        } else if message.text.len() > carlos_proto::source::INLINE_SOURCE_BYTES {
            Event::SourceChanged {
                entry: entry.to_wire(),
                source: session.export_source(message),
            }
        } else {
            Event::TextAppended {
                entry: entry.to_wire(),
                text: message.text.get(from..)?.to_owned(),
            }
        })
    }
}
