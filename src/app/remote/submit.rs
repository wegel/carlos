//! Submit exact composer text after local controls consume their own commands.

use anyhow::Result;
use carlos_proto::commands::Command;
use carlos_proto::session::{Availability, Backend};

use super::transport::Transport;
use super::Device;
use crate::app::input::make_input_area;

impl Device {
    pub(super) fn submit(&mut self, mut link: Option<&mut Transport>) -> Result<()> {
        let text = self.view.input.lines().join("\n");
        if self.local_command(&text) {
            return Ok(());
        }
        if self.lifecycle_command(link.as_deref_mut(), &text)? {
            return Ok(());
        }
        if self.history.rewind_draft.is_some() {
            let selected = self
                .history
                .selection
                .as_ref()
                .is_some_and(|s| s.record.is_some());
            let claude = self
                .client
                .session(&self.session)
                .is_some_and(|r| r.info().backend == Backend::Claude);
            if selected || claude {
                return self.submit_rewind(link, text);
            }
            self.history.rewind_draft = None;
        }
        if self.history.operation.is_some() {
            anyhow::bail!("Preparing edited branch; newer composer draft kept");
        }
        if self.rename_command(link.as_deref_mut(), &text)? {
            return Ok(());
        }
        if self.panel_command(link.as_deref_mut(), &text) {
            return Ok(());
        }
        let link = link.ok_or_else(|| anyhow::anyhow!("Disconnected; F5 reconnects"))?;
        self.submit_current(link, text)
    }

    fn submit_current(&mut self, link: &mut Transport, text: String) -> Result<()> {
        let session = self.session.clone();
        if let Some(info) = self.client.session(&session).map(|replica| replica.info()) {
            if info.availability == Availability::Closed {
                anyhow::bail!("{:?} is closed; draft kept", info.backend);
            }
        }
        if !self.session_can_accept_input(link)
            || (self.view.codex_queue.mode
                && !self.client.session_is_ready(link.connection, &session))
        {
            self.waiting_ready = true;
            self.view.status_hint = None;
            return Ok(());
        }
        if self.view.codex_queue.mode {
            if let Err(error) = self.submit_queue(link, text) {
                self.view.codex_queue.error = Some(error.to_string());
                self.view.codex_queue.show = true;
            }
            return Ok(());
        }
        if self.client.session(&session).is_some_and(|replica| {
            replica
                .approvals()
                .iter()
                .any(|card| card.blocking && !card.submitted)
        }) {
            return Ok(());
        }
        if text.is_empty() {
            return Ok(());
        }
        self.send_user_text(link, text)
    }

    fn send_user_text(&mut self, link: &mut Transport, text: String) -> Result<()> {
        let session = self.session.clone();
        self.history.failed = false;
        self.history.reset_navigation();
        let policy = self.task_send_policy();
        let outgoing = self
            .client
            .submit(link.connection, |operation| Command::Send {
                session,
                operation,
                text,
                policy,
            })?;
        link.send(outgoing)?;
        self.view.input = make_input_area();
        let count = self
            .client
            .session(&self.session)
            .map_or(0, |replica| replica.entry_count());
        self.dirty_from(count);
        Ok(())
    }

    fn session_can_accept_input(&self, link: &Transport) -> bool {
        self.client
            .session_has_current_snapshot(link.connection, &self.session)
            && self.client.session(&self.session).is_some_and(|replica| {
                matches!(
                    replica.info().availability,
                    Availability::Ready | Availability::Starting
                ) || (replica.info().backend == Backend::Grok
                    && replica.info().availability == Availability::Restoring)
            })
    }

    fn panel_command(&mut self, link: Option<&mut Transport>, text: &str) -> bool {
        if text.trim() == "/goal" {
            self.view.input = make_input_area();
            if let Err(error) = self.open_goal(link) {
                self.view.codex_goals.error = Some(error.to_string());
            }
            return true;
        }
        if text.trim() == "/queue" {
            self.view.input = make_input_area();
            if let Err(error) = self.open_queue(link) {
                self.view.codex_queue.error = Some(error.to_string());
            }
            return true;
        }
        false
    }
}
