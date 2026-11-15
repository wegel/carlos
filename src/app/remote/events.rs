//! Match terminal input to explicit client operations and numbered daemon facts.

use anyhow::Result;
use carlos_client::codec::Incoming;
use carlos_client::Effects;
use carlos_proto::commands::Command;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use super::transport::Transport;
use super::Device;
use crate::app::input::make_input_area;
use crate::app::notifications::is_key_press_like;
use crate::app::render_input::{is_newline_enter, normalize_pasted_text, textarea_input_from_key};

impl Device {
    pub(super) fn receive(&mut self, link: &mut Transport, incoming: Incoming) -> Result<bool> {
        self.reading.received(&incoming);
        let failed_sources = self.documents.received(&incoming);
        let was_ready = self.client.session_is_ready(link.connection, &self.session);
        let goal_read = self.client.pending_goal_read(&self.session);
        let task_read = self.client.pending_task_read(&self.session);
        let mut effects = self.client.receive(link.connection, incoming)?;
        for token in failed_sources {
            effects
                .outgoing
                .extend(self.client.close_body(&token).outgoing);
        }
        if self.goals.read.is_none() {
            self.goals.read = goal_read.or_else(|| self.client.pending_goal_read(&self.session));
        }
        if self.tasks.reads.header.is_none() {
            self.tasks.reads.header =
                task_read.or_else(|| self.client.pending_task_read(&self.session));
        }
        let changed = self.apply_effects(link, effects)?;
        let readiness_changed =
            was_ready != self.client.session_is_ready(link.connection, &self.session);
        if readiness_changed {
            self.tasks.paint_dirty.set(true);
        }
        Ok(changed || readiness_changed)
    }

    pub(super) fn apply_effects(
        &mut self,
        link: &mut Transport,
        mut effects: Effects,
    ) -> Result<bool> {
        self.replace_source_frames(effects.welcome.as_ref());
        let mut task_changed = self.apply_task_reads(&mut effects)?;
        task_changed |= self.task_operations_changed(&effects.operations);
        let goal_changed = self.apply_goal_reads(&mut effects)?;
        let queue_changed = self.apply_queue_reads(&mut effects)?;
        let private_changed = !effects.operations.is_empty();
        self.apply_operations(&effects.operations);
        let mut changed = effects.welcome.is_some()
            || private_changed
            || goal_changed
            || queue_changed
            || task_changed
            || !effects.failures.is_empty();
        for outgoing in effects.outgoing {
            link.send(outgoing)?;
        }
        if effects.welcome.is_some() && !self.client.wants_session(&self.session) {
            link.send(self.client.read(
                link.connection,
                Command::Subscribe {
                    session: self.session.clone(),
                    since: None,
                },
            )?)?;
        }
        for body in effects.bodies {
            self.documents.changed(&self.client, &self.session, &body);
            self.dirty_entry(body.entry());
            changed = true;
        }
        for update in effects.sessions {
            if update.session != self.session {
                continue;
            }
            task_changed |= update.changes.tasks || update.changes.reset;
            let ids = self.changed_entries(update.changes);
            for id in ids {
                self.dirty_entry(&id);
            }
            changed = true;
        }
        for failure in effects.failures {
            self.view.status_hint = Some(failure.error.message);
        }

        if self.client.session_is_ready(link.connection, &self.session) {
            self.waiting_ready = false;
        }
        self.settle_history();
        Ok(self.redraw_after_updates(changed, task_changed))
    }

    fn redraw_after_updates(&self, changed: bool, task_changed: bool) -> bool {
        if task_changed {
            self.tasks.paint_dirty.set(true);
        }
        if self.tasks.view.show {
            task_changed
        } else {
            changed
        }
    }

    fn changed_entries(
        &mut self,
        changes: carlos_client::ChangeSet,
    ) -> Vec<carlos_proto::ids::EntryId> {
        if changes.reset || changes.removed_front > 0 || changes.removed_back > 0 {
            self.dirty_from(0);
            if changes.reset {
                self.documents.selection = None;
                self.documents.copy = None;
                self.documents.formatting.clear();
                self.view.viewport.selection = None;
                self.view.render_cache.invalidate_all();
            }
        }
        changes.entries
    }

    fn replace_source_frames(&mut self, welcome: Option<&carlos_client::Welcome>) {
        if matches!(welcome, Some(carlos_client::Welcome::Replaced { .. })) {
            self.documents.replaced();
            self.view.render_cache.forget_all();
            self.view.viewport.selection = None;
        }
    }

    pub(super) fn event(&mut self, link: Option<&mut Transport>, event: Event) -> Result<bool> {
        match event {
            Event::Mouse(mouse)
                if !self.view.codex_goals.show
                    && !self.view.codex_queue.show
                    && !self.tasks.view.show =>
            {
                self.mouse(mouse)?
            }
            Event::Paste(text) => {
                if self.tasks.view.show {
                    // Task views consume paste without changing any hidden draft.
                } else if self.view.codex_goals.show {
                    self.goal_paste(&normalize_pasted_text(&text));
                } else if self.view.codex_queue.show {
                    self.queue_paste(&normalize_pasted_text(&text));
                } else {
                    self.view.input.insert_str(normalize_pasted_text(&text));
                }
            }
            Event::Key(key) if is_key_press_like(key.kind) => return self.key_event(link, key),
            _ => {}
        }
        Ok(false)
    }

    fn key_event(&mut self, mut link: Option<&mut Transport>, key: KeyEvent) -> Result<bool> {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(true);
        }
        if key.code == KeyCode::F(5) {
            if let Some(link) = link.as_deref_mut() {
                self.retry_session(link)?;
            }
            return Ok(false);
        }
        if self.overlay_key(link.as_deref_mut(), key) {
            return Ok(false);
        }
        if self.dictation_key(key) {
            return Ok(false);
        }
        if self.navigation_key(key)? {
            return Ok(false);
        }
        if key.code == KeyCode::Enter && !is_newline_enter(key.modifiers) {
            let result = self.submit(link);
            if let Err(error) = result {
                self.view.status_hint = Some(error.to_string());
            }
        } else if key.code == KeyCode::Esc {
            if let Some(link) = link {
                if let Err(error) = self.interrupt(link) {
                    self.view.status_hint = Some(error.to_string());
                }
            }
        } else {
            self.view.input.input(textarea_input_from_key(key));
        }
        Ok(false)
    }

    fn retry_session(&mut self, link: &mut Transport) -> Result<()> {
        if self.client.is_ready(link.connection)
            && !self.client.session_is_ready(link.connection, &self.session)
        {
            link.send(self.client.read(
                link.connection,
                Command::Subscribe {
                    session: self.session.clone(),
                    since: None,
                },
            )?)?;
            self.waiting_ready = true;
            self.view.status_hint = None;
        }
        Ok(())
    }

    fn overlay_key(&mut self, mut link: Option<&mut Transport>, key: KeyEvent) -> bool {
        match self.task_key(link.as_deref_mut(), key) {
            Ok(true) => return true,
            Err(error) => {
                self.tasks.view.error = Some(error.to_string());
                self.tasks.paint_dirty.set(true);
                return true;
            }
            Ok(false) => {}
        }
        match self.goal_key(link.as_deref_mut(), key) {
            Ok(true) => return true,
            Err(error) => {
                self.view.codex_goals.error = Some(error.to_string());
                return true;
            }
            Ok(false) => {}
        }
        match self.queue_key(link.as_deref_mut(), key) {
            Ok(true) => return true,
            Err(error) => {
                self.view.codex_queue.error = Some(error.to_string());
                return true;
            }
            Ok(false) => {}
        }
        match self.approval_key(link, key) {
            Ok(true) => return true,
            Err(error) => {
                self.view.status_hint = Some(error.to_string());
                return true;
            }
            Ok(false) => {}
        }
        false
    }

    fn submit(&mut self, mut link: Option<&mut Transport>) -> Result<()> {
        let text = self.view.input.lines().join("\n");
        if self.panel_command(link.as_deref_mut(), &text) {
            return Ok(());
        }
        let link = link.ok_or_else(|| anyhow::anyhow!("Disconnected; F5 reconnects"))?;
        let session = self.session.clone();
        if !self.client.session_is_ready(link.connection, &session) {
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
        if self
            .client
            .session(&session)
            .is_some_and(|replica| replica.approvals().iter().any(|card| card.active))
        {
            return Ok(());
        }
        if text.is_empty() {
            return Ok(());
        }
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
            self.open_queue();
            return true;
        }
        self.local_command(text)
    }

    fn interrupt(&mut self, link: &mut Transport) -> Result<()> {
        let turn = self
            .client
            .session(&self.session)
            .and_then(|replica| replica.info().current_turn.clone());
        if let Some(turn) = turn {
            let session = self.session.clone();
            link.send(
                self.client
                    .submit(link.connection, |operation| Command::Interrupt {
                        session,
                        operation,
                        turn,
                    })?,
            )?;
        }
        Ok(())
    }

    pub(super) fn dirty_entry(&mut self, entry: &carlos_proto::ids::EntryId) {
        if let Some(position) = self
            .client
            .session(&self.session)
            .and_then(|replica| replica.entry_position(entry))
        {
            self.view.render_cache.invalidate_entry(
                position,
                &crate::app::render_entry_cache::RenderKey::Entry(
                    carlos_core::session::entry_id::EntryId::from_wire(entry),
                ),
            );
            self.dirty_from(position);
        }
    }

    pub(super) fn dirty_from(&mut self, index: usize) {
        let length = self
            .client
            .session(&self.session)
            .map_or(0, |replica| replica.entry_count())
            + self
                .client
                .operations()
                .iter()
                .filter(|operation| operation.needs_echo())
                .count();
        self.view
            .render_cache
            .mark_transcript_dirty_from(length, index);
    }
}
