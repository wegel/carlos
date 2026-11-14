//! Apply numbered session facts to one client window without retaining native routing or UI state.

use carlos_proto::events::{Event, SessionEvent};
use carlos_proto::ids::{EntryId, Sequence};
use carlos_proto::session::{Approval, Entry, SessionInfo, Snapshot};

use crate::cards::Cards;
use crate::history::HistoryFence;
use crate::rows::{weight, Rows};
use crate::ClientError;

/// Platforms redraw changed rows and invalidate body caches without rescanning the transcript.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ChangeSet {
    pub entries: Vec<EntryId>,
    pub bodies: Vec<EntryId>,
    pub info: bool,
    pub approvals: bool,
    pub reset: bool,
    pub removed_front: usize,
    pub removed_back: usize,
}

/// One bounded chronological window; the daemon keeps older/newer rows outside this copy.
pub struct Replica {
    pub(crate) info: SessionInfo,
    pub(crate) approvals: Cards,
    pub(crate) rows: Rows,
    pub(crate) seq: Sequence,
    pub(crate) older: bool,
    pub(crate) newer: bool,
    pub(crate) stale: bool,
    pub(crate) history: HistoryFence,
}

impl Replica {
    /// Adopt a full snapshot atomically; duplicate ids fail before replacing an existing copy.
    pub fn new(snapshot: Snapshot) -> Result<Self, ClientError> {
        let mut rows = Rows::new(snapshot.entries)?;
        let removed = rows.bound(true)?;
        let older = snapshot.has_older || removed > 0;
        for card in &snapshot.approvals {
            if rows.get(&card.id).is_some() {
                return Err(ClientError::DuplicateEntry(card.id.clone()));
            }
        }
        Ok(Self {
            info: snapshot.info,
            approvals: Cards::new(snapshot.approvals)?,
            rows,
            seq: snapshot.seq,
            older,
            newer: false,
            stale: false,
            history: HistoryFence::new(snapshot.seq),
        })
    }

    /// Borrow current session facts without exposing a mutable core session.
    pub fn info(&self) -> &SessionInfo {
        &self.info
    }
    /// Borrow rows in chronological window order without cloning their source.
    pub fn entries(&self) -> impl DoubleEndedIterator<Item = &Entry> {
        self.rows.iter()
    }
    /// Resolve one stable id in constant time.
    pub fn entry(&self, id: &EntryId) -> Option<&Entry> {
        self.rows.get(id)
    }
    /// Borrow live cards in server order; editor drafts stay outside the replica.
    pub fn approvals(&self) -> &[Approval] {
        self.approvals.entries()
    }
    /// Read the cursor after the last successfully applied shared event.
    pub fn sequence(&self) -> Sequence {
        self.seq
    }
    /// Determine whether the caller needs an older history page.
    pub fn has_older(&self) -> bool {
        self.older
    }
    /// A bounded older window may require a fresh newest snapshot before following new rows.
    pub fn has_newer(&self) -> bool {
        self.newer
    }
    /// A gap prevents later deltas from changing the copy until a snapshot replaces it.
    pub fn is_stale(&self) -> bool {
        self.stale
    }

    /// Replace facts for the same session; a late snapshot cannot roll a healthy copy backward.
    pub fn replace(&mut self, snapshot: Snapshot) -> Result<ChangeSet, ClientError> {
        if snapshot.info.id != self.info.id {
            return Err(ClientError::ForeignSession);
        }
        if snapshot.seq < self.seq {
            return Ok(ChangeSet::default());
        }
        *self = Self::new(snapshot)?;
        Ok(ChangeSet {
            info: true,
            approvals: true,
            reset: true,
            ..Default::default()
        })
    }

    /// Apply one ordered event; duplicate delivery never appends text twice and gaps stay visible.
    pub fn apply(&mut self, event: SessionEvent) -> Result<ChangeSet, ClientError> {
        if event.session != self.info.id {
            return Err(ClientError::ForeignSession);
        }
        if event.seq <= self.seq {
            return Ok(ChangeSet::default());
        }
        if let Event::Reset { snapshot } = event.event {
            if snapshot.seq != event.seq {
                self.stale = true;
                return Err(ClientError::InvalidEnvelope);
            }
            return self.replace(snapshot);
        }
        if self.stale || self.seq.checked_next() != Some(event.seq) {
            self.stale = true;
            return Err(ClientError::SequenceGap {
                session: event.session,
                expected: self.seq.checked_next().unwrap_or(self.seq),
                received: event.seq,
            });
        }
        let result = self.apply_fact(event.event, event.seq);
        if result.is_ok() {
            self.seq = event.seq;
        } else {
            self.stale = true;
        }
        result
    }

    fn apply_fact(&mut self, event: Event, seq: Sequence) -> Result<ChangeSet, ClientError> {
        let mut changed = ChangeSet::default();
        match event {
            Event::EntryAdded { entry } => self.add_entry(entry, &mut changed)?,
            Event::EntryUpdated { entry } => self.replace_entry(entry, seq, &mut changed),
            Event::TextAppended { entry, text } => {
                self.append_text(entry, text, seq, &mut changed)?
            }
            Event::EntryFactsChanged { entry, facts } => {
                self.update_row(entry, seq, &mut changed, |row| {
                    row.kind = facts.kind;
                    row.format = facts.format;
                    row.file_path = facts.file_path;
                    row.done = facts.done;
                    row.tool = facts.tool;
                })
            }
            Event::SourceChanged { entry, source } => {
                changed.bodies.push(entry.clone());
                self.update_row(entry, seq, &mut changed, |row| {
                    row.text.clear();
                    row.source = Some(source);
                });
            }
            Event::OutputChanged {
                entry,
                revision,
                has_output,
            } => {
                changed.bodies.push(entry.clone());
                self.update_row(entry, seq, &mut changed, |row| {
                    if let Some(tool) = &mut row.tool {
                        tool.output_revision = revision;
                        tool.has_output = has_output;
                    }
                });
            }
            Event::InfoChanged { info } => {
                if info.id != self.info.id {
                    return Err(ClientError::ForeignSession);
                }
                self.info = info;
                changed.info = true;
            }
            Event::ApprovalRequested { approval, before } => {
                self.approval(approval, before, &mut changed)?
            }
            Event::ApprovalResolved { approval, .. } => {
                self.approvals.remove(&approval);
                changed.approvals = true;
            }
            Event::Reset { .. } => unreachable!("checked replacement"),
        }
        changed.removed_front = self.rows.bound(true)?;
        self.older |= changed.removed_front > 0;
        Ok(changed)
    }

    fn add_entry(&mut self, entry: Entry, changed: &mut ChangeSet) -> Result<(), ClientError> {
        if self.approvals.contains(&entry.id) {
            return Err(ClientError::DuplicateEntry(entry.id));
        }
        if !self.newer {
            changed.entries.push(entry.id.clone());
            self.rows.push(entry)?;
        }
        Ok(())
    }

    fn replace_entry(&mut self, entry: Entry, seq: Sequence, changed: &mut ChangeSet) {
        let id = entry.id.clone();
        changed.bodies.push(id.clone());
        self.update_row(id, seq, changed, |row| *row = entry);
    }

    fn append_text(
        &mut self,
        entry: EntryId,
        text: String,
        seq: Sequence,
        changed: &mut ChangeSet,
    ) -> Result<(), ClientError> {
        if self.rows.get(&entry).is_some_and(|row| {
            row.source.is_some()
                || row.text.len().saturating_add(text.len())
                    > carlos_proto::source::INLINE_SOURCE_BYTES
        }) {
            return Err(ClientError::InvalidPayload(
                "inline text exceeds its source contract".into(),
            ));
        }
        self.update_row(entry, seq, changed, |row| row.text.push_str(&text));
        Ok(())
    }

    fn update_row(
        &mut self,
        entry: EntryId,
        seq: Sequence,
        changed: &mut ChangeSet,
        update: impl FnOnce(&mut Entry),
    ) {
        if let Some(row) = self.rows.get_mut(&entry) {
            let before = weight(row);
            update(row);
            self.rows.changed(before, &entry);
            changed.entries.push(entry);
        } else {
            self.history.changed_unknown(entry, seq);
        }
    }

    fn approval(
        &mut self,
        approval: Approval,
        before: Option<EntryId>,
        changed: &mut ChangeSet,
    ) -> Result<(), ClientError> {
        if self.rows.get(&approval.id).is_some() {
            return Err(ClientError::DuplicateEntry(approval.id));
        }
        self.approvals.insert(approval, before)?;
        changed.approvals = true;
        Ok(())
    }
}
