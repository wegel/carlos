//! Restore one explicit host context-default read only after a current session subscription.

use carlos_proto::commands::Command;
use carlos_proto::context_defaults::ContextDefaults;
use carlos_proto::ids::SessionId;

use crate::panels::{ContextPanel, PanelKind};
use crate::{Client, ClientError, Effects, ReadReply, ReadResult, RequestFailure};

impl Client {
    /// Open the host-default reader without changing this session's current model or token usage.
    pub fn open_context_panel(&mut self, session: &SessionId) -> Result<Effects, ClientError> {
        let slot = self
            .sessions
            .get_mut(session)
            .filter(|slot| slot.wanted)
            .ok_or(ClientError::ForeignSession)?;
        let mut effects = Effects::default();
        if slot.panels.context.is_none() {
            slot.panels.context = Some(Box::new(ContextPanel {
                waiting: true,
                ..Default::default()
            }));
            effects.panel_changed(session, PanelKind::Context);
        }
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Explicitly read the latest saved host choice without replacing an editor's text.
    pub fn refresh_context_panel(&mut self, session: &SessionId) -> Result<Effects, ClientError> {
        self.cancel_context_read(session);
        let panel = self
            .context_panel_mut(session)
            .ok_or(ClientError::ForeignSession)?;
        panel.result = None;
        panel.waiting = true;
        let mut effects = Effects::default();
        effects.panel_changed(session, PanelKind::Context);
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Borrow this small checked reader independently of a model-list editor.
    pub fn context_panel(&self, session: &SessionId) -> Option<&ContextPanel> {
        self.sessions.get(session)?.panels.context.as_deref()
    }

    /// Move one complete read result into an editor's private saved-choice baseline.
    pub fn take_context_defaults(
        &mut self,
        session: &SessionId,
    ) -> Option<Result<ContextDefaults, String>> {
        self.context_panel_mut(session)?.result.take()
    }

    /// Retire the point read even while offline, preserving all platform-owned draft choices.
    pub fn close_context_panel(&mut self, session: &SessionId) -> Effects {
        self.cancel_context_read(session);
        let mut effects = Effects::default();
        if let Some(slot) = self.sessions.get_mut(session) {
            if slot.panels.context.take().is_some() {
                effects.panel_changed(session, PanelKind::Context);
            }
        }
        effects
    }

    pub(crate) fn context_panel_mut(&mut self, session: &SessionId) -> Option<&mut ContextPanel> {
        self.sessions
            .get_mut(session)?
            .panels
            .context
            .as_deref_mut()
    }

    pub(crate) fn cancel_context_read(&mut self, session: &SessionId) {
        let id = self
            .context_panel_mut(session)
            .and_then(|panel| panel.request.take());
        if let Some(id) = id {
            self.requests.take(&id);
        }
    }

    pub(crate) fn drive_context_panels(&mut self, effects: &mut Effects) {
        let sessions: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, slot)| {
                slot.panels
                    .context
                    .as_ref()
                    .is_some_and(|panel| panel.waiting && panel.request.is_none())
            })
            .map(|(session, _)| session.clone())
            .collect();
        for session in sessions {
            if !self.current_session_is_ready(&session) {
                continue;
            }
            let result = self.read(
                self.current.expect("ready host-default reader"),
                Command::ContextRead {
                    session: session.clone(),
                },
            );
            let panel = self
                .context_panel_mut(&session)
                .expect("open host-default reader");
            panel.waiting = false;
            match result {
                Ok(outgoing) => {
                    panel.request = Some(outgoing.id());
                    effects.outgoing.push(outgoing);
                }
                Err(error) => panel.result = Some(Err(error.to_string())),
            }
            effects.panel_changed(&session, PanelKind::Context);
            self.enforce_fact_budget(&session, effects);
        }
    }

    pub(crate) fn accept_context_panel_reply(
        &mut self,
        result: ReadResult,
        effects: &mut Effects,
    ) -> Option<ReadResult> {
        let ReadReply::ContextDefaults(defaults) = &result.reply else {
            return Some(result);
        };
        let session = defaults.session.clone();
        let Some(panel) = self.context_panel_mut(&session) else {
            return Some(result);
        };
        if result.id.is_none() || panel.request != result.id {
            return Some(result);
        }
        let ReadReply::ContextDefaults(defaults) = result.reply else {
            unreachable!("host default reply")
        };
        panel.request = None;
        panel.result = Some(Ok(defaults));
        effects.panel_changed(&session, PanelKind::Context);
        None
    }

    pub(crate) fn fail_context_panel_read(
        &mut self,
        failure: &RequestFailure,
        effects: &mut Effects,
    ) -> bool {
        let Some(session) = self.sessions.iter().find_map(|(session, slot)| {
            slot.panels
                .context
                .as_ref()
                .filter(|panel| panel.request == failure.id && failure.id.is_some())
                .map(|_| session.clone())
        }) else {
            return false;
        };
        let panel = self
            .context_panel_mut(&session)
            .expect("matching host-default reader");
        panel.request = None;
        panel.result = Some(Err(failure.error.message.clone()));
        effects.panel_changed(&session, PanelKind::Context);
        true
    }
}
