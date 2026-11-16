//! Own selected-task point requests while preserving captured windows and private platform choices.

use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::Command;
use carlos_proto::ids::{SessionId, TaskWindowId};

use crate::panels::task_details::{Pending, MAX_TASK_DETAIL_BYTES};
use crate::panels::{PanelKind, TaskDetailTarget, TaskDetails, TaskFieldRead};
use crate::{ChangeSet, Client, ClientError, Effects};

impl Client {
    /// Read a selected task's previews, reusing an existing reader only for the identical target.
    pub fn open_task_details(
        &mut self,
        session: &SessionId,
        target: TaskDetailTarget,
    ) -> Result<Effects, ClientError> {
        if !self.wants_session(session) {
            return Err(ClientError::ForeignSession);
        }
        if self
            .task_details(session)
            .is_none_or(|panel| panel.target() != &target)
        {
            self.close_task_details(session);
            self.sessions
                .get_mut(session)
                .expect("wanted session")
                .panels
                .task_details = Some(Box::new(TaskDetails::new(target)));
        }
        let mut effects = Effects::default();
        effects.panel_changed(session, PanelKind::TaskDetails);
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Choose bounded field prefixes after inspecting the previews, without importing a UI policy.
    pub fn read_task_fields(
        &mut self,
        session: &SessionId,
        fields: Vec<TaskFieldRead>,
    ) -> Result<Effects, ClientError> {
        if fields.len() > 32
            || fields.iter().enumerate().any(|(index, read)| {
                !(4..=MAX_TASK_DETAIL_BYTES).contains(&read.bytes)
                    || fields[..index]
                        .iter()
                        .any(|other| other.field == read.field)
            })
        {
            return Err(ClientError::InvalidPayload(
                "Task field interest has duplicate fields or invalid byte limits".into(),
            ));
        }
        if self.task_details(session).is_some_and(|panel| {
            panel
                .pending
                .as_ref()
                .is_some_and(|read| read.field.is_some())
        }) {
            self.cancel_task_detail_read(session);
        }
        let panel = self
            .task_details_mut(session)
            .ok_or(ClientError::ForeignSession)?;
        panel.fields = Some(fields);
        panel.dirty = true;
        let mut effects = Effects::default();
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Borrow one device's selected metadata without exposing mutable accepted session facts.
    pub fn task_details(&self, session: &SessionId) -> Option<&TaskDetails> {
        self.sessions.get(session)?.panels.task_details.as_deref()
    }

    /// Retire pure point reads without closing their parent task window or changing a private draft.
    pub fn close_task_details(&mut self, session: &SessionId) -> Effects {
        self.cancel_task_detail_read(session);
        let mut effects = Effects::default();
        if let Some(slot) = self.sessions.get_mut(session) {
            if slot.panels.task_details.take().is_some() {
                effects.panel_changed(session, PanelKind::TaskDetails);
            }
        }
        effects
    }

    pub(crate) fn task_details_mut(&mut self, session: &SessionId) -> Option<&mut TaskDetails> {
        self.sessions
            .get_mut(session)?
            .panels
            .task_details
            .as_deref_mut()
    }

    pub(crate) fn cancel_task_detail_read(&mut self, session: &SessionId) {
        let pending = self
            .task_details_mut(session)
            .and_then(|panel| panel.pending.take());
        if let Some(pending) = pending {
            self.requests.take(&pending.id);
        }
    }

    /// Keep accepted fields readonly when their exact parent window releases its native rights.
    pub(crate) fn retire_task_window_details(
        &mut self,
        session: &SessionId,
        window: &TaskWindowId,
        effects: &mut Effects,
    ) {
        if self
            .task_details(session)
            .is_none_or(|panel| panel.target.window.as_ref() != Some(window))
        {
            return;
        }
        self.cancel_task_detail_read(session);
        let panel = self.task_details_mut(session).expect("captured details");
        panel.stale = true;
        panel.dirty = false;
        effects.panel_changed(session, PanelKind::TaskDetails);
    }

    pub(crate) fn wake_task_details(&mut self, session: &SessionId, changes: &ChangeSet) {
        let Some(slot) = self.sessions.get_mut(session) else {
            return;
        };
        let Some(panel) = &mut slot.panels.task_details else {
            return;
        };
        panel.dirty |= changes.tasks || changes.reset || changes.info;
        if panel.failed_sequence.is_some_and(|failed| {
            slot.replica
                .as_ref()
                .is_some_and(|replica| replica.task_sequence() > failed)
        }) {
            panel.failed_sequence = None;
            panel.error = None;
            panel.dirty = true;
        }
    }

    pub(crate) fn drive_task_details(&mut self, effects: &mut Effects) {
        let sessions: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, slot)| {
                slot.panels
                    .task_details
                    .as_ref()
                    .is_some_and(|panel| panel.dirty)
            })
            .map(|(session, _)| session.clone())
            .collect();
        for session in sessions {
            if !self.current_session_is_ready(&session) {
                continue;
            }
            self.task_details_mut(&session).expect("open details").dirty = false;
            self.request_task_detail(&session, effects);
            self.enforce_fact_budget(&session, effects);
        }
    }

    pub(crate) fn task_detail_target_is_current(&self, session: &SessionId) -> bool {
        let Some(panel) = self.task_details(session) else {
            return false;
        };
        !panel.stale
            && (panel.target.window.is_some()
                || self
                    .session(session)
                    .and_then(|replica| replica.tasks())
                    .is_some_and(|facts| facts.revision == panel.target.expected))
    }

    fn request_task_detail(&mut self, session: &SessionId, effects: &mut Effects) {
        if !self.task_detail_target_is_current(session) {
            return;
        }
        let panel = self.task_details(session).expect("open details");
        if panel.pending.is_some() || panel.error.is_some() {
            return;
        }
        let next = panel.next_field();
        let command = match detail_command(session, panel, next) {
            Ok(Some(command)) => command,
            Ok(None) => return,
            Err(error) => {
                self.task_details_mut(session).expect("open details").error =
                    Some(error.to_string());
                effects.panel_changed(session, PanelKind::TaskDetails);
                return;
            }
        };
        let sequence = self
            .session(session)
            .expect("current session")
            .task_sequence();
        let result = self.read(self.current.expect("ready detail reader"), command);
        let panel = self.task_details_mut(session).expect("open details");
        match result {
            Ok(outgoing) => {
                panel.pending = Some(Pending {
                    id: outgoing.id(),
                    field: next.map(|(field, _, _)| field),
                    offset: next.map_or(0, |(_, offset, _)| offset),
                    sequence,
                });
                effects.outgoing.push(outgoing);
            }
            Err(error) => {
                panel.error = Some(error.to_string());
                panel.failed_sequence = Some(sequence);
            }
        }
        effects.panel_changed(session, PanelKind::TaskDetails);
    }
}

fn detail_command(
    session: &SessionId,
    panel: &TaskDetails,
    next: Option<(carlos_proto::tasks::TaskField, usize, usize)>,
) -> Result<Option<Command>, ClientError> {
    let target = panel.target();
    if panel.value().is_none() {
        return Ok(Some(Command::TaskDetail {
            session: session.clone(),
            window: target.window.clone(),
            expected: target.expected.clone(),
            task: target.task.clone(),
        }));
    }
    let Some((field, offset, left)) = next else {
        return Ok(None);
    };
    let bytes: usize = panel
        .value()
        .expect("checked detail")
        .properties
        .iter()
        .map(|property| property.text.len())
        .sum();
    if bytes >= MAX_TASK_DETAIL_BYTES {
        return Err(ClientError::InvalidPayload(
            "Task detail reached the native 8 MiB view".into(),
        ));
    }
    Ok(Some(Command::TaskText {
        session: session.clone(),
        window: target.window.clone(),
        expected: target.expected.clone(),
        task: target.task.clone(),
        field,
        offset: ByteOffset::new(offset as u64),
        limit: Some(left.min(MAX_TASK_DETAIL_BYTES - bytes).clamp(4, 65536) as u32),
    }))
}
