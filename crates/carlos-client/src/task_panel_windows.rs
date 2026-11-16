//! Keep old task windows until a candidate header and first page prove a complete replacement.

use carlos_proto::commands::Command;
use carlos_proto::ids::SessionId;
use carlos_proto::session::Backend;
use carlos_proto::tasks::TaskPage;

use crate::panels::tasks::WindowRead;
use crate::panels::{PanelKind, TaskPageWindow};
use crate::{Client, ClientError, Effects};

impl Client {
    /// Explicitly read an older Codex window without replacing the current page while it loads.
    pub fn open_older_task_window(&mut self, session: &SessionId) -> Result<Effects, ClientError> {
        if !self.current_session_is_ready(session) {
            return Err(ClientError::InvalidConnection);
        }
        if self
            .session(session)
            .is_none_or(|replica| replica.info().backend != Backend::Codex)
        {
            return Err(ClientError::InvalidPayload(
                "Older native task windows are available only for Codex".into(),
            ));
        }
        let panel = self
            .task_panel(session)
            .ok_or(ClientError::ForeignSession)?;
        if panel.windows.stale || panel.windows.pending.is_some() || !panel.wanted || panel.paused {
            return Err(ClientError::InvalidPayload(
                "The older task window retired or its read is already pending".into(),
            ));
        }
        let expected = panel
            .pages
            .revision()
            .cloned()
            .ok_or(ClientError::StaleTasks)?;
        let after = panel
            .windows
            .current
            .as_ref()
            .map(|window| window.window.clone());
        let outgoing = self.read(
            self.current.expect("ready tasks"),
            Command::TaskWindowOpen {
                session: session.clone(),
                expected,
                after,
            },
        )?;
        let panel = self.task_panel_mut(session).expect("open tasks");
        panel.windows.pending = Some(WindowRead {
            request: Some(outgoing.id()),
            ..Default::default()
        });
        panel.windows.error = None;
        let mut effects = Effects::default();
        effects.outgoing.push(outgoing);
        effects.panel_changed(session, PanelKind::Tasks);
        Ok(effects)
    }

    pub(crate) fn retire_task_windows(&mut self, session: &SessionId, latest: bool) {
        let Some(panel) = self.task_panel_mut(session) else {
            return;
        };
        let mut cancel = None;
        if let Some(pending) = panel.windows.pending.take() {
            if let Some(header) = pending.header {
                panel.windows.close.push(header.window);
                cancel = pending.request;
            } else if let Some(id) = pending.request {
                panel.windows.retired.push(id);
            }
        }
        if let Some(window) = &panel.windows.current {
            if !panel.windows.stale {
                panel.windows.close.push(window.window.clone());
            }
            panel.windows.stale = true;
        }
        if latest {
            panel.windows.current = None;
            panel.windows.stale = false;
        }
        panel.windows.error = None;
        panel.current = false;
        panel.dirty = true;
        if let Some(id) = cancel {
            self.requests.take(&id);
        }
    }

    pub(crate) fn drain_task_window_closes(
        &mut self,
        session: &SessionId,
        effects: &mut Effects,
    ) -> bool {
        loop {
            let Some(window) = self
                .task_panel(session)
                .and_then(|panel| panel.windows.close.first())
                .cloned()
            else {
                return true;
            };
            let command = Command::TaskWindowClose {
                session: session.clone(),
                window: Some(window),
            };
            match self.read(self.current.expect("ready window close"), command) {
                Ok(outgoing) => {
                    self.task_panel_mut(session)
                        .expect("tasks model")
                        .windows
                        .close
                        .remove(0);
                    effects.outgoing.push(outgoing);
                }
                Err(_) => return false,
            }
        }
    }

    pub(crate) fn drive_task_window_candidate(
        &mut self,
        session: &SessionId,
        effects: &mut Effects,
    ) {
        let Some(pending) = self
            .task_panel(session)
            .and_then(|panel| panel.windows.pending.as_ref())
        else {
            return;
        };
        if pending.request.is_some() {
            return;
        }
        let Some(header) = &pending.header else {
            return;
        };
        let page = !header.reading;
        let command = if page {
            Command::TaskList {
                session: session.clone(),
                window: Some(header.window.clone()),
                expected: Some(header.revision.clone()),
                after: None,
                limit: Some(32),
                refresh: false,
            }
        } else {
            Command::TaskWindowRead {
                session: session.clone(),
                window: header.window.clone(),
            }
        };
        match self.read(self.current.expect("ready task window"), command) {
            Ok(outgoing) => {
                let pending = self
                    .task_panel_mut(session)
                    .expect("open tasks")
                    .windows
                    .pending
                    .as_mut()
                    .expect("candidate window");
                pending.request = Some(outgoing.id());
                effects.outgoing.push(outgoing);
            }
            Err(error) => self.fail_task_window_candidate(session, &error.to_string()),
        }
        effects.panel_changed(session, PanelKind::Tasks);
    }

    pub(crate) fn accept_task_window_page(
        &mut self,
        session: &SessionId,
        page: TaskPage,
        effects: &mut Effects,
    ) {
        let pending = self
            .task_panel_mut(session)
            .expect("owned candidate")
            .windows
            .pending
            .take()
            .expect("candidate window");
        let header = pending.header.expect("completed older header");
        let mut pages = TaskPageWindow::default();
        let mut selected = None;
        pages.reset(header.revision.clone(), false);
        let result = if page.revision != header.revision || !page.known || page.reading {
            Err(ClientError::StaleTasks)
        } else {
            pages.accept(page, 0, header.rows as usize, &mut selected)
        };
        if let Err(error) = result {
            let panel = self.task_panel_mut(session).expect("open tasks");
            panel.windows.close.push(header.window);
            panel.windows.error = Some(format!(
                "Older tasks unavailable; previous page retained: {error}"
            ));
            panel.dirty = true;
            return;
        }
        self.cancel_task_page(session);
        self.cancel_task_header(session);
        self.release_task_watch(session);
        let panel = self.task_panel_mut(session).expect("open tasks");
        panel.count = header.rows as usize;
        if let Some(previous) = panel.windows.current.replace(header) {
            panel.windows.close.push(previous.window);
        }
        panel.windows.stale = false;
        panel.windows.error = None;
        panel.pages = pages;
        panel.selected = selected;
        panel.charge_pages();
        panel.dirty = true;
        effects.panel_changed(session, PanelKind::TaskWindow);
    }

    pub(crate) fn fail_task_window_candidate(&mut self, session: &SessionId, message: &str) {
        let panel = self.task_panel_mut(session).expect("owned candidate");
        if let Some(header) = panel
            .windows
            .pending
            .take()
            .and_then(|pending| pending.header)
        {
            panel.windows.close.push(header.window);
        }
        panel.windows.error = Some(format!(
            "Older tasks unavailable; previous page retained: {message}"
        ));
        panel.dirty = true;
    }
}
