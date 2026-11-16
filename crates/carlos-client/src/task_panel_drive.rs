//! Drive current task headers and one bounded page while preserving selected opaque identities.

use carlos_proto::commands::Command;
use carlos_proto::ids::{SessionId, TaskId, TaskRevision};
use carlos_proto::tasks::MAX_TASK_ROWS;

use crate::panels::tasks::{Failure, PageRead};
use crate::panels::PanelKind;
use crate::{ChangeSet, Client, ClientError, Effects, Outbound};

impl Client {
    /// Require live subscribed facts as well as the task panel's accepted page and window checks.
    pub fn task_panel_is_current(&self, session: &SessionId) -> bool {
        if !self.current_session_is_ready(session) {
            return false;
        }
        let Some(panel) = self.task_panel(session).filter(|panel| panel.wanted) else {
            return false;
        };
        if let Some(window) = &panel.windows.current {
            return !panel.windows.stale && panel.pages.revision() == Some(&window.revision);
        }
        self.session(session)
            .and_then(|replica| replica.tasks())
            .is_some_and(|facts| {
                let count = u64::from(facts.counts.agents)
                    + u64::from(facts.counts.processes)
                    + u64::from(facts.counts.tasks);
                facts.known
                    && !facts.reading
                    && !facts.closed
                    && count <= MAX_TASK_ROWS as u64
                    && panel.pages.revision() == Some(&facts.revision)
            })
    }

    pub(crate) fn record_task_panel_read(
        &mut self,
        session: &SessionId,
        outgoing: &Outbound,
        effects: &mut Effects,
    ) {
        if let Some(panel) = self
            .task_panel_mut(session)
            .filter(|panel| panel.wanted && !panel.paused && panel.windows.current.is_none())
        {
            panel.reads.header = Some(outgoing.id());
            panel.reads.header_error = None;
            panel.refresh = false;
            panel.dirty = true;
            effects.panel_changed(session, PanelKind::Tasks);
        }
    }

    pub(crate) fn wake_task_panel(&mut self, session: &SessionId, changes: &ChangeSet) {
        let Some(slot) = self.sessions.get_mut(session) else {
            return;
        };
        let Some(panel) = &mut slot.panels.tasks else {
            return;
        };
        let reached = slot.replica.as_ref().is_some_and(|replica| {
            panel
                .reads
                .future_header
                .as_ref()
                .is_some_and(|header| header.seq <= replica.sequence())
                || panel
                    .reads
                    .future_page
                    .as_ref()
                    .is_some_and(|(_, page)| page.seq <= replica.sequence())
                || panel
                    .reads
                    .failure
                    .as_ref()
                    .is_some_and(|failed| failed.sequence < replica.task_sequence())
        });
        panel.dirty |= changes.tasks || changes.reset || changes.info || reached;
    }

    pub(crate) fn drive_task_panels(&mut self, effects: &mut Effects) {
        let sessions: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, slot)| slot.panels.tasks.as_ref().is_some_and(|panel| panel.dirty))
            .map(|(session, _)| session.clone())
            .collect();
        for session in sessions {
            if !self.current_session_is_ready(&session) {
                continue;
            }
            if !self.drain_task_window_closes(&session, effects) {
                continue;
            }
            self.task_panel_mut(&session).expect("task model").dirty = false;
            self.settle_task_panel_header(&session, effects);
            self.settle_task_panel_page(&session, effects);
            if self
                .task_panel(&session)
                .is_none_or(|panel| !panel.wanted || panel.paused)
            {
                continue;
            }
            self.drive_task_window_candidate(&session, effects);
            self.drive_current_task_list(&session, effects);
            if let Some(panel) = self.task_panel_mut(&session) {
                panel.charge_pages();
            }
            self.enforce_fact_budget(&session, effects);
        }
    }

    fn drive_current_task_list(&mut self, session: &SessionId, effects: &mut Effects) {
        let Some(panel) = self.task_panel(session) else {
            return;
        };
        if panel.windows.stale {
            return;
        }
        if self.needs_task_panel_header(session) {
            self.request_task_panel_header(session, effects);
            return;
        }
        if panel.reads.header.is_some()
            || panel.reads.page.is_some()
            || panel.reads.future_header.is_some()
            || panel.reads.future_page.is_some()
        {
            return;
        }
        if panel.refresh {
            self.task_panel_mut(session).expect("open tasks").refresh = false;
            self.request_task_panel_page(session, None, None, 0, true, effects);
            return;
        }
        let Some((revision, count, _)) = self.current_task_view(session) else {
            if self
                .task_panel(session)
                .is_some_and(|panel| panel.read_error().is_some())
            {
                effects.panel_changed(session, PanelKind::Tasks);
            }
            return;
        };
        let sequence = self
            .session(session)
            .expect("current session")
            .task_sequence();
        let panel = self.task_panel_mut(session).expect("open tasks");
        panel.count = count;
        if panel
            .reads
            .failure
            .as_ref()
            .is_some_and(|failed| failed.sequence < sequence)
        {
            panel.reads.failure = None;
        }
        if panel.reads.header_error.is_some() || panel.reads.failure.is_some() {
            return;
        }
        if panel.pages.revision() != Some(&revision) {
            panel
                .pages
                .reset(revision.clone(), panel.selected.is_some());
            effects.panel_changed(session, PanelKind::Tasks);
        }
        if let Some((start, after)) = panel.pages.next() {
            self.request_task_panel_page(session, Some(revision), after, start, false, effects);
        }
    }

    fn needs_task_panel_header(&self, session: &SessionId) -> bool {
        self.task_panel(session)
            .is_some_and(|panel| panel.windows.current.is_none())
            && (!self.task_watch_is_active(session)
                || self
                    .session(session)
                    .is_none_or(|replica| replica.tasks().is_none()))
    }

    fn current_task_view(&mut self, session: &SessionId) -> Option<(TaskRevision, usize, bool)> {
        let panel = self.task_panel(session)?;
        if let Some(window) = &panel.windows.current {
            return Some((
                window.revision.clone(),
                window.rows as usize,
                !panel.windows.stale,
            ));
        }
        let facts = self.session(session)?.tasks()?;
        if !facts.known || facts.reading {
            return None;
        }
        let count = u64::from(facts.counts.agents)
            + u64::from(facts.counts.processes)
            + u64::from(facts.counts.tasks);
        if count > MAX_TASK_ROWS as u64 {
            let panel = self.task_panel_mut(session).expect("open tasks");
            panel.reads.header_error = Some("Task count exceeds the supported native view".into());
            return None;
        }
        Some((facts.revision.clone(), count as usize, !facts.closed))
    }

    fn request_task_panel_header(&mut self, session: &SessionId, effects: &mut Effects) {
        let panel = self.task_panel(session).expect("open tasks");
        if panel.reads.header.is_some()
            || panel.reads.future_header.is_some()
            || panel.reads.header_error.is_some()
        {
            return;
        }
        match self.read(
            self.current.expect("ready tasks"),
            Command::TaskSubscribe {
                session: session.clone(),
            },
        ) {
            Ok(outgoing) => {
                self.record_task_panel_read(session, &outgoing, effects);
                effects.outgoing.push(outgoing);
            }
            Err(error) => {
                self.task_panel_mut(session)
                    .expect("open tasks")
                    .reads
                    .header_error = Some(error.to_string());
                effects.panel_changed(session, PanelKind::Tasks);
            }
        }
    }

    fn request_task_panel_page(
        &mut self,
        session: &SessionId,
        expected: Option<TaskRevision>,
        after: Option<TaskId>,
        start: usize,
        refresh: bool,
        effects: &mut Effects,
    ) {
        let panel = self.task_panel(session).expect("open tasks");
        let window = panel
            .windows
            .current
            .as_ref()
            .map(|window| window.window.clone());
        let sequence = self
            .session(session)
            .expect("current session")
            .task_sequence();
        let command = Command::TaskList {
            session: session.clone(),
            window,
            expected,
            after,
            limit: Some(32),
            refresh,
        };
        let result = self.read(self.current.expect("ready tasks"), command);
        let panel = self.task_panel_mut(session).expect("open tasks");
        match result {
            Ok(outgoing) => {
                panel.reads.page = Some(PageRead {
                    id: outgoing.id(),
                    start,
                    sequence,
                });
                effects.outgoing.push(outgoing);
            }
            Err(error) => {
                panel.reads.failure = Some(Failure {
                    sequence,
                    text: error.to_string(),
                })
            }
        }
        effects.panel_changed(session, PanelKind::Tasks);
    }

    fn settle_task_panel_header(&mut self, session: &SessionId, effects: &mut Effects) {
        let Some(snapshot) = self
            .task_panel_mut(session)
            .and_then(|panel| panel.reads.future_header.take())
        else {
            return;
        };
        if self
            .session(session)
            .is_some_and(|replica| replica.sequence() < snapshot.seq)
        {
            self.task_panel_mut(session)
                .expect("open tasks")
                .reads
                .future_header = Some(snapshot);
            return;
        }
        if let Err(error) = self.apply_task_header(&snapshot, effects) {
            if error != ClientError::StaleTasks {
                self.task_panel_mut(session)
                    .expect("open tasks")
                    .reads
                    .header_error = Some(error.to_string());
            }
        }
        effects.panel_changed(session, PanelKind::Tasks);
    }

    fn settle_task_panel_page(&mut self, session: &SessionId, effects: &mut Effects) {
        let Some((read, page)) = self
            .task_panel_mut(session)
            .and_then(|panel| panel.reads.future_page.take())
        else {
            return;
        };
        let older = self
            .task_panel(session)
            .is_some_and(|panel| panel.windows.current.is_some());
        if !older {
            if let Err(error) = self.check_task_page(&page) {
                if self
                    .session(session)
                    .is_some_and(|replica| replica.sequence() < page.seq)
                {
                    self.task_panel_mut(session)
                        .expect("open tasks")
                        .reads
                        .future_page = Some((read, page));
                } else {
                    let panel = self.task_panel_mut(session).expect("open tasks");
                    panel.pages.invalidate();
                    if error != ClientError::StaleTasks {
                        panel.reads.failure = Some(Failure {
                            sequence: read.sequence,
                            text: error.to_string(),
                        });
                    }
                    effects.panel_changed(session, PanelKind::Tasks);
                }
                return;
            }
        }
        if !page.known || page.reading {
            return;
        }
        let Some((_, count, _)) = self.current_task_view(session) else {
            return;
        };
        let panel = self.task_panel_mut(session).expect("open tasks");
        panel.count = count;
        if panel.pages.revision() != Some(&page.revision) {
            panel
                .pages
                .reset(page.revision.clone(), panel.selected.is_some());
        }
        if let Err(error) = panel
            .pages
            .accept(page, read.start, count, &mut panel.selected)
        {
            panel.reads.failure = Some(Failure {
                sequence: read.sequence,
                text: error.to_string(),
            });
        }
        panel.charge_pages();
        effects.panel_changed(session, PanelKind::Tasks);
    }
}
