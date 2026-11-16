//! Open, move and pause one device's task-list reader without moving terminal keys or stop intent.

use carlos_proto::ids::{SessionId, TaskId};

use crate::panels::{PanelKind, TaskPanel};
use crate::{Client, ClientError, Effects};

impl Client {
    /// Open the task list, preserving its prior selected identity unless a new initial target is given.
    pub fn open_task_panel(
        &mut self,
        session: &SessionId,
        selected: Option<TaskId>,
    ) -> Result<Effects, ClientError> {
        if selected.is_some()
            && self
                .task_panel(session)
                .is_some_and(|panel| panel.selected != selected)
        {
            self.cancel_task_page(session);
        }
        let slot = self
            .sessions
            .get_mut(session)
            .filter(|slot| slot.wanted)
            .ok_or(ClientError::ForeignSession)?;
        let panel = slot.panels.tasks.get_or_insert_with(Box::default);
        if selected.is_some() && selected != panel.selected {
            panel.selected = selected;
            panel.pages = Default::default();
            panel.charge_pages();
        }
        panel.wanted = true;
        panel.paused = false;
        panel.reads.header_error = None;
        panel.windows.error = None;
        panel.dirty = true;
        let mut effects = Effects::default();
        effects.panel_changed(session, PanelKind::Tasks);
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Borrow the checked task window while the platform keeps its scroll and private stop consent.
    pub fn task_panel(&self, session: &SessionId) -> Option<&TaskPanel> {
        self.sessions.get(session)?.panels.tasks.as_deref()
    }

    /// Apply explicit arrow movement, preserving ordered moves while refreshed counts are unknown.
    pub fn move_task_panel(
        &mut self,
        session: &SessionId,
        direction: isize,
    ) -> Result<Effects, ClientError> {
        self.cancel_task_page(session);
        let panel = self
            .task_panel_mut(session)
            .ok_or(ClientError::ForeignSession)?;
        panel
            .pages
            .navigate(direction, panel.count, &mut panel.selected)?;
        panel.windows.error = None;
        panel.dirty = true;
        panel.charge_pages();
        let mut effects = Effects::default();
        effects.panel_changed(session, PanelKind::Tasks);
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Explicitly leave older readonly rows and request the latest native task list.
    pub fn refresh_task_panel(&mut self, session: &SessionId) -> Result<Effects, ClientError> {
        if self.task_panel(session).is_none() {
            return Err(ClientError::ForeignSession);
        }
        self.retire_task_windows(session, true);
        self.cancel_task_page(session);
        let panel = self.task_panel_mut(session).expect("open tasks");
        panel.wanted = true;
        panel.paused = false;
        panel.refresh = true;
        panel.reads.header_error = None;
        panel.windows.error = None;
        panel.dirty = true;
        let mut effects = Effects::default();
        effects.panel_changed(session, PanelKind::Tasks);
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Pause native list reads while a platform focuses child activity; retained rows remain available.
    pub fn pause_task_panel(
        &mut self,
        session: &SessionId,
        paused: bool,
    ) -> Result<Effects, ClientError> {
        let panel = self
            .task_panel_mut(session)
            .ok_or(ClientError::ForeignSession)?;
        panel.paused = paused;
        panel.dirty = true;
        if paused {
            self.release_task_watch(session);
            self.cancel_task_header(session);
            self.cancel_task_page(session);
        }
        let mut effects = Effects::default();
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Close owned reads and native windows, retaining selected rows readonly for a later reopen.
    pub fn close_task_panel(&mut self, session: &SessionId) -> Effects {
        self.retire_task_windows(session, false);
        self.cancel_task_page(session);
        self.cancel_task_header(session);
        self.release_task_watch(session);
        if let Some(panel) = self.task_panel_mut(session) {
            panel.wanted = false;
            panel.current = false;
            panel.dirty = true;
        }
        let mut effects = Effects::default();
        effects.panel_changed(session, PanelKind::Tasks);
        self.drive(&mut effects);
        effects
    }

    pub(crate) fn task_panel_mut(&mut self, session: &SessionId) -> Option<&mut TaskPanel> {
        self.sessions.get_mut(session)?.panels.tasks.as_deref_mut()
    }

    pub(crate) fn cancel_task_header(&mut self, session: &SessionId) {
        if let Some(panel) = self.task_panel_mut(session) {
            let id = panel.reads.header.take();
            panel.reads.future_header = None;
            if let Some(id) = id {
                self.requests.take(&id);
            }
        }
    }

    pub(crate) fn cancel_task_page(&mut self, session: &SessionId) {
        if let Some(panel) = self.task_panel_mut(session) {
            let id = panel.reads.page.take().map(|read| read.id);
            panel.reads.future_page = None;
            panel.reads.failure = None;
            if let Some(id) = id {
                self.requests.take(&id);
            }
        }
    }

    pub(crate) fn cancel_task_panel_reads(&mut self, session: &SessionId) {
        self.cancel_task_header(session);
        self.cancel_task_page(session);
        self.retire_task_windows(session, true);
        let retired = self
            .task_panel_mut(session)
            .map(|panel| std::mem::take(&mut panel.windows.retired))
            .unwrap_or_default();
        for id in retired {
            self.requests.take(&id);
        }
    }
}
