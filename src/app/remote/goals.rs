//! Keep each terminal's goal draft and keys separate from shared native goal facts.

use anyhow::Result;
use carlos_client::{ClientError, Effects, ReadReply};
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::goals::{GoalChange, GoalFacts, GoalKnowledge};
use carlos_proto::ids::GoalRevision;
use carlos_proto::submissions::ClientOperation;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use serde_json::{json, Value};

use super::transport::Transport;
use super::Device;
use crate::app::codex_goal_state::GoalDraft;
use crate::app::codex_goal_text::{panel_body, PanelFacts};
use crate::app::codex_goal_view::{cursor_for, draw_panel, edit_draft, editable, paste_draft};
use crate::app::view_models::TerminalSize;

/// Freeze the exact native revision when the user chooses a goal action.
pub(super) struct GoalAction {
    pub(super) expected: GoalRevision,
    pub(super) change: Option<GoalChange>,
}

/// Retain only this device's pending commands and consent, never another shared goal copy.
pub(super) struct Goals {
    pub(super) operation: Option<ClientOperation>,
    pub(super) ralph: Option<ClientOperation>,
    pub(super) confirm: Option<GoalAction>,
    pub(super) read: Option<RequestId>,
    pub(super) size: TerminalSize,
    pub(super) clear_editor: bool,
    read_error: Option<String>,
}

impl Default for Goals {
    fn default() -> Self {
        Self {
            operation: None,
            clear_editor: false,
            read_error: None,
            ralph: None,
            confirm: None,
            read: None,
            size: TerminalSize {
                width: 0,
                height: 0,
            },
        }
    }
}

impl Device {
    /// Merge checked goal reads and keep their failures in this local panel.
    pub(super) fn apply_goal_reads(&mut self, effects: &mut Effects) -> Result<bool> {
        let mut changed = false;
        for result in std::mem::take(&mut effects.replies) {
            if self.goals.read.is_some() && result.id == self.goals.read {
                self.goals.read = None;
                self.goals.read_error = None;
                changed = true;
            }
            if let ReadReply::Goal(snapshot) = result.reply {
                match self.client.merge_goal(snapshot) {
                    Ok(merged) => {
                        effects.sessions.extend(merged.sessions);
                        effects.outgoing.extend(merged.outgoing);
                        effects.failures.extend(merged.failures);
                        changed = true;
                    }
                    Err(ClientError::StaleGoal) => {}
                    Err(error) => return Err(error.into()),
                }
            } else {
                effects.replies.push(result);
            }
        }
        for failure in &effects.failures {
            if self.goals.read.is_some() && failure.id == self.goals.read {
                self.goals.read = None;
                self.goals.read_error = Some(failure.error.message.clone());
                changed = true;
            }
        }
        Ok(changed)
    }

    /// Borrow the one accepted goal copy held by the shared Client.
    pub(super) fn goal_facts(&self) -> Option<&GoalFacts> {
        self.client.session(&self.session)?.goal()
    }

    /// Keep this editor locked while its own write or a shared native write is pending.
    pub(super) fn goal_pending(&self) -> bool {
        self.goals
            .operation
            .as_ref()
            .and_then(|id| self.client.operations().get(id))
            .is_some_and(|record| {
                matches!(
                    record.state,
                    carlos_proto::submissions::OperationState::Pending
                        | carlos_proto::submissions::OperationState::Queued
                )
            })
            || self.goal_facts().is_some_and(|facts| facts.pending)
    }

    /// Open the local goal panel and start its explicit native read lifetime.
    pub(super) fn open_goal(&mut self, link: Option<&mut Transport>) -> Result<()> {
        self.view.codex_goals.show = true;
        self.view.codex_goals.scroll = 0;
        self.watch_goal_panel(link)
    }

    fn watch_goal_panel(&mut self, link: Option<&mut Transport>) -> Result<()> {
        let effects = self.client.watch_goal(&self.session)?;
        self.goals.read = self.client.pending_goal_read(&self.session);
        if let Some(link) = link {
            for outgoing in effects.outgoing {
                link.send(outgoing)?;
            }
        }
        Ok(())
    }

    /// Ask the daemon to refresh accepted native facts without writing a goal.
    pub(super) fn refresh_goal(&mut self, link: Option<&mut Transport>) -> Result<()> {
        let link = link.ok_or_else(|| anyhow::anyhow!("Disconnected; F5 reconnects"))?;
        self.watch_goal_panel(Some(&mut *link))?;
        if self.goals.read.is_none() {
            self.read_goal(link)?;
        }
        Ok(())
    }

    fn read_goal(&mut self, link: &mut Transport) -> Result<()> {
        if self.goals.read.is_none() {
            let outgoing = self.client.read(
                link.connection,
                Command::GoalRead {
                    session: self.session.clone(),
                    refresh: true,
                },
            )?;
            self.goals.read = Some(outgoing.id());
            link.send(outgoing)?;
        }
        Ok(())
    }

    /// Consume goal controls before the composer and approval shortcuts.
    pub(super) fn goal_key(&mut self, link: Option<&mut Transport>, key: KeyEvent) -> Result<bool> {
        if !self.view.codex_goals.show {
            if key.code == KeyCode::Char('g') && key.modifiers.contains(KeyModifiers::ALT) {
                self.open_goal(link)?;
                return Ok(true);
            }
            return Ok(false);
        }
        if key.code == KeyCode::F(4) {
            self.close_goal_panel(link)?;
            self.next_question();
        } else if key.code == KeyCode::Char('r') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.toggle_ralph(link)?;
        } else if key.code == KeyCode::Esc {
            self.goal_back(link)?;
        } else if self.view.codex_goals.confirm.is_some() {
            match key.code {
                KeyCode::Char('y') => self.confirm_goal(link)?,
                KeyCode::Char('n') => self.cancel_goal_confirmation(),
                _ => self.goal_scroll(key.code),
            }
        } else if self.view.codex_goals.editing {
            self.goal_editor_key(link, key)?;
        } else {
            self.goal_overview_key(link, key)?;
        }
        Ok(true)
    }

    fn goal_back(&mut self, link: Option<&mut Transport>) -> Result<()> {
        if self.view.codex_goals.confirm.is_some() {
            self.cancel_goal_confirmation();
        } else if self.view.codex_goals.editing {
            self.view.codex_goals.editing = false;
        } else {
            self.close_goal_panel(link)?;
        }
        Ok(())
    }

    fn close_goal_panel(&mut self, link: Option<&mut Transport>) -> Result<()> {
        self.view.codex_goals.show = false;
        self.goals.read = None;
        let effects = self.client.close_goal(&self.session);
        if let Some(link) = link {
            for outgoing in effects.outgoing {
                link.send(outgoing)?;
            }
        }
        Ok(())
    }

    /// Discard only this terminal's unsent consent.
    pub(super) fn cancel_goal_confirmation(&mut self) {
        self.view.codex_goals.confirm = None;
        self.goals.confirm = None;
    }

    fn goal_overview_key(&mut self, link: Option<&mut Transport>, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('e') => self.edit_goal(),
            KeyCode::Char('p') => self.goal_status(link, false)?,
            KeyCode::Char('a') => self.goal_status(link, true)?,
            KeyCode::Char('d') => self.prepare_goal_clear()?,
            KeyCode::Char('r') => self.refresh_goal(link)?,
            _ => self.goal_scroll(key.code),
        }
        Ok(())
    }

    fn goal_scroll(&mut self, key: KeyCode) {
        let scroll = &mut self.view.codex_goals.scroll;
        match key {
            KeyCode::PageUp => *scroll = scroll.saturating_sub(5),
            KeyCode::PageDown => *scroll = scroll.saturating_add(5),
            _ => {}
        }
    }

    fn edit_goal(&mut self) {
        if self.goal_pending() {
            self.view.codex_goals.error =
                Some("Goal change pending; draft stays locked until the reply".into());
        } else if !self
            .goal_facts()
            .is_some_and(|facts| facts.knowledge == GoalKnowledge::Known)
        {
            self.view.codex_goals.error =
                Some("Read the current Codex goal before editing; r refreshes".into());
        } else {
            if self.view.codex_goals.editor.is_none() {
                self.view.codex_goals.editor = Some(GoalDraft::new(&self.goal_value()));
            }
            self.view.codex_goals.editing = true;
        }
    }

    fn goal_editor_key(&mut self, link: Option<&mut Transport>, key: KeyEvent) -> Result<()> {
        if self.goal_pending() {
            return Ok(());
        }
        let size = self.goals.size;
        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if editable(size) {
                self.save_goal(link)?;
            }
        } else if let Some(draft) = self.view.codex_goals.editor.as_mut() {
            edit_draft(draft, key, size);
        }
        Ok(())
    }

    /// Paste into this terminal's chosen goal field when its editor is usable.
    pub(super) fn goal_paste(&mut self, text: &str) {
        let state = &self.view.codex_goals;
        if !state.editing
            || state.confirm.is_some()
            || self.goal_pending()
            || !editable(self.goals.size)
        {
            return;
        }
        if let Some(draft) = self.view.codex_goals.editor.as_mut() {
            paste_draft(draft, text);
        }
    }

    /// Adapt bounded typed values to the existing pure local goal widgets.
    pub(super) fn goal_value(&self) -> Value {
        self.goal_facts().and_then(|facts| facts.goal.as_ref()).map_or(Value::Null, |goal| json!({
            "objective":goal.objective, "status":goal.status,
            "tokenBudget":goal.token_budget.map(|value| value.get()),
            "tokensUsed":goal.tokens_used.get(), "timeUsedSeconds":goal.time_used_seconds.get(),
        }))
    }

    /// Render shared facts beside this terminal's independent goal controls.
    pub(super) fn draw_goal(&self, buffer: &mut Buffer, size: TerminalSize) -> (usize, usize) {
        let facts = self.goal_facts();
        let knowledge = match facts.map(|facts| facts.knowledge) {
            Some(GoalKnowledge::Known) => "Known",
            Some(GoalKnowledge::Unavailable) => "Unavailable/disabled",
            _ => "Unknown",
        };
        let ralph = if self.goals.ralph.is_some()
            || self.client.session(&self.session).is_some_and(|session| {
                session
                    .info()
                    .status
                    .as_ref()
                    .is_some_and(|status| status.ralph.toggle_pending)
            }) {
            "toggle pending"
        } else if self
            .client
            .session(&self.session)
            .is_some_and(|replica| replica.info().ralph)
        {
            "on"
        } else {
            "off"
        };
        let goal = self.goal_value();
        let mut lines = panel_body(
            &self.view.codex_goals,
            PanelFacts {
                goal: &goal,
                knowledge,
                reading: self.goals.read.is_some() || facts.is_some_and(|facts| facts.reading),
                pending: self.goal_pending(),
                read_error: self
                    .goals
                    .read_error
                    .as_deref()
                    .or_else(|| facts.and_then(|facts| facts.read_error.as_deref())),
                error: self.view.codex_goals.error.as_deref(),
                notice: facts.map_or("", |facts| facts.notice.as_str()),
                thread: self.session.as_str(),
                ralph,
            },
            size,
        );
        if let Some(status) = self
            .view
            .status_hint
            .as_deref()
            .filter(|status| status.starts_with("F5 reconnects:"))
        {
            lines.insert(0, status.to_string());
        }
        draw_panel(buffer, &self.view.codex_goals, lines, size);
        cursor_for(&self.view.codex_goals, size)
    }
}

#[cfg(test)]
#[path = "goals_private_tests.rs"]
mod private_tests;
