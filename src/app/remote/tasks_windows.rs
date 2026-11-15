//! Keep older task-list windows private and replace the visible page only after a complete read.

use anyhow::{ensure, Result};
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::{TaskRevision, TaskWindowId};
use carlos_proto::session::Backend;
use carlos_proto::task_windows::TaskWindow;
use carlos_proto::tasks::TaskPage;

use super::transport::Transport;
use super::Device;

#[derive(Default)]
struct Pending {
    request: Option<RequestId>,
    header: Option<TaskWindow>,
    page: bool,
}

#[derive(Default)]
pub(super) struct Windows {
    pub(super) current: Option<TaskWindow>,
    pub(super) stale: bool,
    pending: Option<Pending>,
    close: Vec<TaskWindowId>,
    close_latest: Vec<carlos_client::Outbound>,
    close_all: bool,
}

impl Windows {
    pub(super) fn disconnected(&mut self) {
        self.close.clear();
        self.close_latest.clear();
        self.close_all = false;
    }
}

impl Device {
    pub(super) fn task_window_id(&self) -> Option<TaskWindowId> {
        self.tasks
            .windows
            .current
            .as_ref()
            .map(|window| window.window.clone())
    }

    pub(super) fn task_view_revision(&self) -> Option<&TaskRevision> {
        self.tasks
            .windows
            .current
            .as_ref()
            .map(|window| &window.revision)
            .or_else(|| self.task_facts().map(|facts| &facts.revision))
    }

    pub(super) fn task_view_is_current(&self) -> bool {
        if self.tasks.windows.current.is_some() {
            !self.tasks.windows.stale
                && self.tasks.pages.revision.as_ref() == self.task_view_revision()
        } else {
            self.task_facts()
                .is_some_and(|facts| facts.known && !facts.reading && !facts.closed)
                && self.tasks.pages.revision.as_ref() == self.task_view_revision()
        }
    }

    pub(super) fn open_older_tasks(&mut self, link: Option<&mut Transport>) -> Result<()> {
        if self.task_backend() != Some(Backend::Codex) {
            return Ok(());
        }
        let link =
            link.ok_or_else(|| anyhow::anyhow!("Reconnect before reading older native tasks"))?;
        ensure!(
            self.client.session_is_ready(link.connection, &self.session),
            "Wait for accepted session facts"
        );
        ensure!(
            !self.tasks.windows.stale,
            "This older window retired; r reads the latest native tasks"
        );
        ensure!(
            self.tasks.windows.pending.is_none(),
            "An older native task window is already reading"
        );
        let expected = self
            .tasks
            .pages
            .revision
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Wait for the current task page"))?;
        let outgoing = self.client.read(
            link.connection,
            Command::TaskWindowOpen {
                session: self.session.clone(),
                expected,
                after: self.task_window_id(),
            },
        )?;
        self.tasks.windows.pending = Some(Pending {
            request: Some(outgoing.id()),
            ..Default::default()
        });
        self.tasks.view.error = None;
        self.tasks.paint_dirty.set(true);
        link.send(outgoing)
    }

    pub(super) fn drive_task_windows(&mut self, link: &mut Transport) -> Result<bool> {
        for outgoing in std::mem::take(&mut self.tasks.windows.close_latest) {
            link.send(outgoing)?;
        }
        if std::mem::take(&mut self.tasks.windows.close_all) {
            self.tasks.windows.close.clear();
            link.send(self.client.read(
                link.connection,
                Command::TaskWindowClose {
                    session: self.session.clone(),
                    window: None,
                },
            )?)?;
        }
        for window in std::mem::take(&mut self.tasks.windows.close) {
            link.send(self.client.read(
                link.connection,
                Command::TaskWindowClose {
                    session: self.session.clone(),
                    window: Some(window),
                },
            )?)?;
        }
        let Some(pending) = &self.tasks.windows.pending else {
            return Ok(false);
        };
        if pending.request.is_some() {
            return Ok(false);
        }
        let Some(header) = &pending.header else {
            return Ok(false);
        };
        let page = !header.reading;
        let command = if page {
            Command::TaskList {
                session: self.session.clone(),
                window: Some(header.window.clone()),
                expected: Some(header.revision.clone()),
                after: None,
                limit: Some(32),
                refresh: false,
            }
        } else {
            Command::TaskWindowRead {
                session: self.session.clone(),
                window: header.window.clone(),
            }
        };
        let outgoing = self.client.read(link.connection, command)?;
        let pending = self
            .tasks
            .windows
            .pending
            .as_mut()
            .expect("candidate window");
        pending.request = Some(outgoing.id());
        pending.page = page;
        link.send(outgoing)?;
        Ok(true)
    }

    pub(super) fn older_tasks_reading(&self) -> bool {
        self.tasks.windows.pending.is_some()
    }

    pub(super) fn accept_task_window(&mut self, id: Option<RequestId>, header: TaskWindow) -> bool {
        let Some(pending) = &mut self.tasks.windows.pending else {
            return false;
        };
        if id.is_none() || pending.request.as_ref() != id.as_ref() || pending.page {
            return false;
        }
        pending.request = None;
        if let Some(error) = header.read_error.as_ref().filter(|_| !header.known) {
            self.tasks.view.error = Some(error.clone());
            self.tasks.windows.close.push(header.window);
            self.tasks.windows.pending = None;
        } else {
            pending.header = Some(header);
        }
        true
    }

    pub(super) fn accept_task_window_page(
        &mut self,
        id: Option<&RequestId>,
        page: &TaskPage,
    ) -> Result<bool> {
        let Some(pending) = &self.tasks.windows.pending else {
            return Ok(false);
        };
        if id.is_none() || !pending.page || pending.request.as_ref() != id {
            return Ok(false);
        }
        let pending = self
            .tasks
            .windows
            .pending
            .take()
            .expect("matching older page");
        let header = pending.header.expect("completed older header");
        ensure!(
            page.revision == header.revision && page.known && !page.reading,
            "Older task page changed while reading"
        );
        if let Some(previous) = self.tasks.windows.current.replace(header.clone()) {
            self.tasks.windows.close.push(previous.window);
        }
        self.tasks.windows.stale = false;
        self.tasks.reads.clear();
        self.tasks.pages.reset(page.revision.clone(), false);
        self.tasks.view.selected = None;
        self.tasks.pages.accept(
            page.clone(),
            0,
            header.rows as usize,
            &mut self.tasks.view.selected,
        )?;
        self.tasks.view.scroll = 0;
        self.clear_task_details();
        self.tasks.view.error = None;
        self.tasks.paint_dirty.set(true);
        // Releasing this terminal's latest-list timer leaves other terminals' interests intact.
        let effects = self.client.close_tasks(&self.session);
        for outgoing in effects.outgoing {
            // apply_effects sends these frames after all task replies have been matched.
            self.tasks.windows.close_latest.push(outgoing);
        }
        Ok(true)
    }

    pub(super) fn fail_task_window(&mut self, id: &RequestId, message: &str) -> bool {
        if !self
            .tasks
            .windows
            .pending
            .as_ref()
            .is_some_and(|pending| pending.request.as_ref() == Some(id))
        {
            return false;
        }
        if let Some(header) = self
            .tasks
            .windows
            .pending
            .take()
            .and_then(|pending| pending.header)
        {
            self.tasks.windows.close.push(header.window);
        }
        self.tasks.view.error = Some(format!(
            "Older tasks unavailable; previous page retained: {message}"
        ));
        true
    }

    pub(super) fn retire_task_windows(&mut self, latest: bool) {
        self.tasks.windows.close_all |= self.tasks.windows.pending.is_some()
            || self.tasks.windows.current.is_some() && !self.tasks.windows.stale;
        if let Some(pending) = self.tasks.windows.pending.take() {
            self.tasks.reads.retire(pending.request);
            if let Some(header) = pending.header {
                self.tasks.windows.close.push(header.window);
            }
        }
        if let Some(header) = &self.tasks.windows.current {
            if !self.tasks.windows.stale {
                self.tasks.windows.close.push(header.window.clone());
            }
            self.tasks.windows.stale = true;
        }
        if latest {
            self.tasks.windows.current = None;
            self.tasks.windows.stale = false;
        }
    }
}
