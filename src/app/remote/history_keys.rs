//! Route recall and the idle Escape chord without changing another terminal's editor.

use std::time::Instant;

use anyhow::{bail, Result};
use carlos_proto::commands::Command;
use carlos_proto::prompts::{PromptDirection, PromptSource};
use crossterm::event::{KeyCode, KeyEvent};

use super::transport::Transport;
use super::Device;

impl Device {
    pub(super) fn history_key(&mut self, link: Option<&mut Transport>, key: KeyEvent) -> bool {
        let result = match key.code {
            KeyCode::Enter => return false,
            KeyCode::Up | KeyCode::Down => self.navigate_prompt(link, key.code == KeyCode::Up),
            KeyCode::F(4) if self.history.rewind_draft.is_some() => self.switch_prompt_source(link),
            KeyCode::Esc if key.modifiers.is_empty() => self.history_escape(link),
            _ => {
                self.view.reset_esc_chord();
                self.history.cancel_read();
                if self.history.rewind_draft.is_none() {
                    self.history.reset_navigation();
                }
                return false;
            }
        };
        if let Err(error) = result {
            self.view.status_hint = Some(error.to_string());
        }
        true
    }

    pub(super) fn navigate_prompt(
        &mut self,
        link: Option<&mut Transport>,
        older: bool,
    ) -> Result<()> {
        self.view.reset_esc_chord();
        if self.history.pending.is_some() {
            if self.history.moves.len() == 128 {
                bail!("History navigation is busy; wait for the selected prompt");
            }
            self.history.moves.push_back(older);
            return Ok(());
        }
        let link =
            link.ok_or_else(|| anyhow::anyhow!("Disconnected; draft kept; F5 reconnects"))?;
        let selection = self.history.selection.as_ref();
        let source = selection.map_or(PromptSource::Human, |selection| selection.source);
        let anchor = selection.and_then(|selection| selection.record.as_ref());
        if !older
            && (anchor.is_none()
                || source == PromptSource::Unknown
                    && !selection.is_some_and(|selection| selection.has_newer))
        {
            return Ok(());
        }
        let direction = if anchor.is_none() {
            PromptDirection::Latest
        } else if older {
            PromptDirection::Older
        } else {
            PromptDirection::Newer
        };
        self.select_prompt(link, source, direction)
    }

    fn switch_prompt_source(&mut self, link: Option<&mut Transport>) -> Result<()> {
        let link =
            link.ok_or_else(|| anyhow::anyhow!("Disconnected; draft kept; F5 reconnects"))?;
        let source = if self
            .history
            .selection
            .as_ref()
            .is_some_and(|selection| selection.source == PromptSource::Unknown)
        {
            PromptSource::Human
        } else {
            PromptSource::Unknown
        };
        self.history.cancel_read();
        self.select_prompt(link, source, PromptDirection::Latest)
    }

    fn history_escape(&mut self, link: Option<&mut Transport>) -> Result<()> {
        let now = Instant::now();
        if let Some(draft) = self.history.rewind_draft.take() {
            self.history.reset_navigation();
            self.set_composer(&draft);
            self.view.viewport.auto_follow_bottom = true;
            self.view.status_hint = None;
            self.view.esc_armed_at = None;
            return Ok(());
        }
        if self.main_is_running()
            || self
                .client
                .session(&self.session)
                .is_some_and(|session| session.info().ralph)
        {
            self.view.suppress_esc_chord_after_interrupt(now);
            let link = link.ok_or_else(|| anyhow::anyhow!("Disconnected; F5 reconnects"))?;
            self.stop_ralph_for_escape(link)?;
            return self.interrupt(link);
        }
        if !self.view.register_escape_press(now) {
            return Ok(());
        }
        self.history.reset_navigation();
        if !self.view.input.is_empty() {
            self.set_composer("");
            return Ok(());
        }
        let link =
            link.ok_or_else(|| anyhow::anyhow!("Disconnected; draft kept; F5 reconnects"))?;
        self.history.rewind_draft = Some(String::new());
        self.view.viewport.selection = None;
        self.view.viewport.auto_follow_bottom = false;
        self.select_prompt(link, PromptSource::Human, PromptDirection::Latest)
    }

    fn stop_ralph_for_escape(&mut self, link: &mut Transport) -> Result<()> {
        if !self.client.session_is_ready(link.connection, &self.session) {
            bail!("Waiting for current ready session facts; no stop was sent");
        }
        if self
            .client
            .session(&self.session)
            .is_some_and(|session| session.info().ralph)
        {
            let session = self.session.clone();
            link.send(
                self.client
                    .submit(link.connection, |operation| Command::SetRalph {
                        session,
                        operation,
                        on: false,
                        options: None,
                    })?,
            )?;
        }
        Ok(())
    }

    fn main_is_running(&self) -> bool {
        use carlos_proto::task_overview::TaskMainState;
        self.client.session(&self.session).is_some_and(|session| {
            session.info().current_turn.is_some()
                || session
                    .info()
                    .status
                    .as_ref()
                    .and_then(|status| status.activity.as_ref())
                    .is_some_and(|activity| matches!(activity.state, TaskMainState::Running))
        })
    }
}

#[cfg(test)]
#[path = "history_keys_tests.rs"]
mod tests;
