//! Open, advance and close only this panel's native child reader through typed existing commands.

use carlos_proto::commands::Command;
use carlos_proto::ids::SessionId;

use crate::panels::{PanelKind, TaskActivityPanel, TaskDetailTarget};
use crate::{Client, ClientError, Effects};

impl Client {
    /// Explicitly open a captured child; reconnect never reopens this native reader automatically.
    pub fn open_task_activity(
        &mut self,
        session: &SessionId,
        target: TaskDetailTarget,
    ) -> Result<Effects, ClientError> {
        if !self.current_session_is_ready(session) {
            return Err(ClientError::InvalidConnection);
        }
        if self.task_activity(session).is_none() {
            self.sessions
                .get_mut(session)
                .ok_or(ClientError::ForeignSession)?
                .panels
                .task_activity = Some(Box::default());
        }
        self.prepare_activity_close(session);
        let panel = self.task_activity_mut(session).expect("open activity");
        if panel
            .target
            .as_ref()
            .is_none_or(|old| old.task != target.task)
        {
            panel.accepted = None;
        }
        panel.target = Some(target);
        panel.header = None;
        panel.error = None;
        panel.failed_revision = None;
        panel.wanted = true;
        panel.stale = false;
        panel.dirty = true;
        let mut effects = Effects::default();
        // Drain closes before opening a replacement, preserving the original RPC order.
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Borrow one device's accepted child display and current native reader status.
    pub fn task_activity(&self, session: &SessionId) -> Option<&TaskActivityPanel> {
        self.sessions.get(session)?.panels.task_activity.as_deref()
    }

    /// Reload or read older child items only on an explicit platform action.
    pub fn advance_task_activity(
        &mut self,
        session: &SessionId,
        older: bool,
    ) -> Result<Effects, ClientError> {
        if !self.current_session_is_ready(session) {
            return Err(ClientError::InvalidConnection);
        }
        let panel = self
            .task_activity(session)
            .ok_or(ClientError::ForeignSession)?;
        let header = panel
            .header
            .as_ref()
            .filter(|header| {
                panel.wanted && !panel.stale && !header.reading && panel.requests.advance.is_none()
            })
            .ok_or_else(|| {
                ClientError::InvalidPayload(
                    "The child reader retired or its native read is still pending".into(),
                )
            })?;
        let outgoing = self.read(
            self.current.expect("ready child"),
            Command::TaskActivityAdvance {
                session: session.clone(),
                activity: header.activity.clone(),
                older,
            },
        )?;
        let panel = self.task_activity_mut(session).expect("open child");
        panel.requests.advance = Some(outgoing.id());
        panel.failed_revision = None;
        panel.error = None;
        let mut effects = Effects::default();
        effects.outgoing.push(outgoing);
        effects.panel_changed(session, PanelKind::TaskActivity);
        Ok(effects)
    }

    /// Release this panel's native interest exactly once while keeping accepted records readonly.
    pub fn close_task_activity(&mut self, session: &SessionId) -> Effects {
        self.prepare_activity_close(session);
        let mut effects = Effects::default();
        effects.panel_changed(session, PanelKind::TaskActivity);
        self.drive(&mut effects);
        effects
    }

    fn prepare_activity_close(&mut self, session: &SessionId) {
        self.retire_task_activity_reads(session);
        if let Some(panel) = self.task_activity_mut(session) {
            panel.close_current();
        }
    }

    pub(crate) fn task_activity_mut(
        &mut self,
        session: &SessionId,
    ) -> Option<&mut TaskActivityPanel> {
        self.sessions
            .get_mut(session)?
            .panels
            .task_activity
            .as_deref_mut()
    }

    pub(crate) fn retire_task_activity_reads(&mut self, session: &SessionId) {
        let ids = self
            .task_activity_mut(session)
            .map(|panel| panel.retire())
            .unwrap_or_default();
        for id in ids {
            self.requests.take(&id);
        }
    }

    pub(crate) fn cancel_task_activity_reads(&mut self, session: &SessionId) {
        self.retire_task_activity_reads(session);
        let retired = self
            .task_activity_mut(session)
            .map(|panel| std::mem::take(&mut panel.retired))
            .unwrap_or_default();
        for id in retired {
            self.requests.take(&id);
        }
    }
}
