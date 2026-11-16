//! Settle owned queue pages against numbered facts before requesting the next bounded page.

use carlos_proto::commands::Command;
use carlos_proto::ids::{QueueEntryId, QueueRevision, SessionId};
use carlos_proto::queue::MAX_PAGE_ENTRIES;

use crate::panels::queue_reads::{PageRead, ReadFailure};
use crate::panels::PanelKind;
use crate::{ChangeSet, Client, ClientError, Effects};

impl Client {
    pub(crate) fn wake_queue_panel(&mut self, session: &SessionId, changes: &ChangeSet) {
        let Some(slot) = self.sessions.get_mut(session) else {
            return;
        };
        let Some(panel) = &mut slot.panels.queue else {
            return;
        };
        let reached = panel.reads.future.as_ref().is_some_and(|(_, page)| {
            slot.replica
                .as_ref()
                .is_some_and(|replica| replica.sequence() >= page.seq)
        });
        panel.dirty |= changes.queue || changes.reset || changes.info || reached;
    }

    pub(crate) fn drive_queue_panels(&mut self, effects: &mut Effects) {
        let sessions: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, slot)| slot.panels.queue.as_ref().is_some_and(|panel| panel.dirty))
            .map(|(session, _)| session.clone())
            .collect();
        for session in sessions {
            if !self.current_session_is_ready(&session) {
                continue;
            }
            if let Some(panel) = self.queue_panel_mut(&session) {
                panel.dirty = false;
            }
            self.settle_queue_page(&session, effects);
            self.request_queue_page(&session, effects);
            if let Some(panel) = self.queue_panel_mut(&session) {
                panel.recount();
            }
            self.enforce_fact_budget(&session, effects);
        }
    }

    fn settle_queue_page(&mut self, session: &SessionId, effects: &mut Effects) {
        let future = self
            .queue_panel_mut(session)
            .and_then(|panel| panel.reads.future.take());
        let Some((read, page)) = future else { return };
        match self.apply_queue_header(&page, effects) {
            Ok(()) => {}
            Err(ClientError::StaleQueue) => {
                let ahead = self
                    .session(session)
                    .is_some_and(|replica| replica.sequence() < page.seq);
                if let Some(panel) = self.queue_panel_mut(session) {
                    if ahead {
                        panel.reads.future = Some((read, page));
                    } else {
                        panel.pages.invalidate();
                    }
                }
                return;
            }
            Err(error) => {
                self.queue_read_failed(session, read.revision, error.to_string());
                return;
            }
        }
        let Some(panel) = self.queue_panel_mut(session) else {
            return;
        };
        if page.queue.known && !page.queue.reading {
            if panel.pages.revision() != Some(&page.queue.revision) {
                panel
                    .pages
                    .reset(page.queue.revision.clone(), panel.selection.is_some());
            }
            if let Err(error) = panel.pages.accept(page, read.start, &mut panel.selection) {
                panel.reads.failure = Some(ReadFailure {
                    revision: panel.pages.revision().cloned(),
                    message: error.to_string(),
                });
            }
        }
        effects.panel_changed(session, PanelKind::Queue);
    }

    fn request_queue_page(&mut self, session: &SessionId, effects: &mut Effects) {
        let facts = self.session(session).and_then(|replica| replica.queue());
        let revision = facts.map(|facts| facts.revision.clone());
        let known = facts.is_some_and(|facts| facts.known && !facts.reading);
        let Some(panel) = self.queue_panel_mut(session) else {
            return;
        };
        if panel.reads.busy() {
            return;
        }
        if panel.refresh {
            self.issue_queue_page(session, None, None, 0, true, effects);
            return;
        }
        let Some(revision) = revision.filter(|_| known) else {
            return;
        };
        panel.reads.retire_failure(&revision);
        if panel.reads.failure.is_some() {
            return;
        }
        if panel.pages.revision() != Some(&revision) {
            panel
                .pages
                .reset(revision.clone(), panel.selection.is_some());
            effects.panel_changed(session, PanelKind::Queue);
        }
        if let Some((start, after)) = panel.pages.next() {
            self.issue_queue_page(session, Some(revision), after, start, false, effects);
        } else {
            self.drive_queue_input(session, effects);
        }
    }

    fn issue_queue_page(
        &mut self,
        session: &SessionId,
        expected: Option<QueueRevision>,
        after: Option<QueueEntryId>,
        start: usize,
        refresh: bool,
        effects: &mut Effects,
    ) {
        let revision = expected.clone().or_else(|| {
            self.session(session)
                .and_then(|replica| replica.queue())
                .map(|facts| facts.revision.clone())
        });
        let connection = self.current.expect("ready queue panel");
        let result = self.read(
            connection,
            Command::QueueRead {
                session: session.clone(),
                expected,
                after,
                limit: Some(MAX_PAGE_ENTRIES as u16),
                refresh,
            },
        );
        match result {
            Ok(outgoing) => {
                let panel = self.queue_panel_mut(session).expect("open queue panel");
                panel.refresh = false;
                panel.reads.request = Some(PageRead {
                    id: outgoing.id(),
                    start,
                    refresh,
                    revision,
                });
                effects.outgoing.push(outgoing);
            }
            Err(error) => {
                if let Some(panel) = self.queue_panel_mut(session) {
                    panel.refresh = false;
                }
                self.queue_read_failed(session, revision, error.to_string());
            }
        }
        effects.panel_changed(session, PanelKind::Queue);
    }

    pub(crate) fn queue_read_failed(
        &mut self,
        session: &SessionId,
        revision: Option<QueueRevision>,
        message: String,
    ) {
        if let Some(panel) = self.queue_panel_mut(session) {
            panel.reads.failure = Some(ReadFailure { revision, message });
            panel.dirty = true;
            panel.recount();
        }
    }
}
