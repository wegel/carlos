//! Match terminal input to explicit client operations and numbered daemon facts.

use anyhow::Result;
use carlos_client::codec::Incoming;
use carlos_client::Effects;
use carlos_proto::commands::Command;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::transport::Transport;
use super::Device;
use crate::app::notifications::is_key_press_like;
use crate::app::render_input::{is_newline_enter, textarea_input_from_key};

impl Device {
    pub(super) fn receive(&mut self, link: &mut Transport, incoming: Incoming) -> Result<bool> {
        self.reading.received(&incoming);
        let failed_sources = self.documents.received(&incoming);
        let was_ready = self.client.session_is_ready(link.connection, &self.session);
        let had_question_facts = self.question_facts_are_current();
        let mut effects = self.client.receive(link.connection, incoming)?;
        for token in failed_sources {
            effects
                .outgoing
                .extend(self.client.close_body(&token).outgoing);
        }
        let changed = self.apply_effects(link, effects)?;
        let readiness_changed =
            was_ready != self.client.session_is_ready(link.connection, &self.session);
        if readiness_changed {
            self.tasks.paint_dirty.set(true);
        }
        let question_readiness_changed = had_question_facts != self.question_facts_are_current();
        if question_readiness_changed {
            self.sync_questions(false);
        }
        Ok(changed || readiness_changed || question_readiness_changed)
    }

    pub(super) fn apply_effects(
        &mut self,
        link: &mut Transport,
        mut effects: Effects,
    ) -> Result<bool> {
        self.replace_source_frames(effects.welcome.as_ref());
        self.question_effects(&effects);
        let history_changed = self.apply_prompt_reads(link, &mut effects)?;
        let mut task_changed = self.apply_task_reads(&mut effects);
        task_changed |= self.task_operations_changed(&effects.operations);
        let goal_changed = self.goal_model_changed(&effects);
        let settings_changed = self.apply_settings_reads(&mut effects);
        let queue_changed = self.apply_queue_model(&effects)?;
        let private_changed = !effects.operations.is_empty();
        self.apply_operations(&effects.operations);
        let mut changed = history_changed
            || effects.welcome.is_some()
            || private_changed
            || goal_changed
            || settings_changed
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

        self.settle_ready_history(link);
        Ok(self.redraw_after_updates(changed, task_changed))
    }

    fn settle_ready_history(&mut self, link: &Transport) {
        if self.client.session_is_ready(link.connection, &self.session) {
            self.waiting_ready = false;
        }
        self.settle_history();
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
                self.history.reset_navigation();
                self.history.rewind_draft = None;
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
            if let Some(operation) = self.history.operation.take() {
                self.retain_failed_edit_text(&operation);
                self.history.failed = true;
            }
            self.history.reset_navigation();
            self.settings_replaced();
            self.documents.replaced();
            self.view.render_cache.forget_all();
            self.view.viewport.selection = None;
        }
    }

    pub(super) fn event(&mut self, link: Option<&mut Transport>, event: Event) -> Result<bool> {
        if self.view.viewport.show_help && !matches!(&event, Event::Key(_) | Event::Resize(_, _)) {
            return Ok(false);
        }
        match event {
            Event::Mouse(mouse)
                if self.selected_approval().is_some()
                    && !self.view.runtime_settings.show_model_settings
                    && !self.view.codex_goals.show
                    && !self.view.codex_queue.show
                    && !self.tasks.view.show =>
            {
                self.question_mouse(mouse);
            }
            Event::Mouse(mouse)
                if !self.view.codex_goals.show
                    && !self.view.codex_queue.show
                    && !self.tasks.view.show =>
            {
                self.mouse(mouse)?
            }
            Event::Paste(text) => self.paste_text(text),
            Event::Key(key) if key.code == KeyCode::Esc && key.kind == KeyEventKind::Repeat => {}
            Event::Key(key) if is_key_press_like(key.kind) => return self.key_event(link, key),
            _ => {}
        }
        Ok(false)
    }

    fn key_event(&mut self, mut link: Option<&mut Transport>, key: KeyEvent) -> Result<bool> {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(true);
        }
        if self.metrics_key(key) || self.help_key(key) {
            return Ok(false);
        }
        if key.code == KeyCode::F(5) {
            if self.recover_saved_draft() {
                return Ok(false);
            }
            if let Some(link) = link.as_deref_mut() {
                self.retry_session(link)?;
            }
            return Ok(false);
        }
        if key.code == KeyCode::F(4)
            && self.history.rewind_draft.is_some()
            && self.selected_approval().is_none()
        {
            self.history_key(link.as_deref_mut(), key);
            return Ok(false);
        }
        if self.settings_key(link.as_deref_mut(), key)?
            || self.overlay_key(link.as_deref_mut(), key)
        {
            return Ok(false);
        }
        if self.dictation_key(key) {
            return Ok(false);
        }
        if self.mobile_key(key) {
            return Ok(false);
        }
        if self.navigation_key(key)? {
            return Ok(false);
        }
        if self.ralph_key(link.as_deref_mut(), key) {
            return Ok(false);
        }
        if self.history_key(link.as_deref_mut(), key) {
            return Ok(false);
        }
        if key.code == KeyCode::Enter && !is_newline_enter(key.modifiers) {
            let result = self.submit(link);
            if let Err(error) = result {
                self.view.status_hint = Some(error.to_string());
            }
        } else {
            self.view.input.input(textarea_input_from_key(key));
        }
        Ok(false)
    }

    pub(super) fn retry_session(&mut self, link: &mut Transport) -> Result<()> {
        if self
            .client
            .session_has_current_snapshot(link.connection, &self.session)
        {
            if let Some(info) = self
                .client
                .session(&self.session)
                .map(|replica| replica.info())
            {
                if info.availability == carlos_proto::session::Availability::Closed {
                    if info.backend == carlos_proto::session::Backend::Codex {
                        return self.native_reconnect(link);
                    }
                    self.waiting_ready = false;
                    self.view.status_hint = None;
                    return Ok(());
                }
            }
        }
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
        match self.question_key(link, key) {
            Ok(true) => return true,
            Err(error) => {
                self.question_error(error.to_string());
                return true;
            }
            Ok(false) => {}
        }
        false
    }

    pub(super) fn interrupt(&mut self, link: &mut Transport) -> Result<()> {
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
