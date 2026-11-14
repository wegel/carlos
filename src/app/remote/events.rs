//! Match terminal input to explicit client operations and numbered daemon facts.

use anyhow::Result;
use carlos_client::codec::Incoming;
use carlos_client::Effects;
use carlos_proto::commands::Command;
use crossterm::event::{Event, KeyCode, KeyModifiers};

use super::transport::Transport;
use super::Device;
use crate::app::input::make_input_area;
use crate::app::notifications::is_key_press_like;
use crate::app::render_input::{is_newline_enter, normalize_pasted_text, textarea_input_from_key};

impl Device {
    pub(super) fn receive(&mut self, link: &mut Transport, incoming: Incoming) -> Result<bool> {
        let effects = self.client.receive(link.connection, incoming)?;
        self.apply_effects(link, effects)
    }

    fn apply_effects(&mut self, link: &mut Transport, effects: Effects) -> Result<bool> {
        let private_changed = !effects.operations.is_empty();
        self.apply_operations(&effects.operations);
        let mut changed =
            effects.welcome.is_some() || private_changed || !effects.failures.is_empty();
        for outgoing in effects.outgoing {
            link.send(outgoing)?;
        }
        if effects.welcome.is_some() && self.client.session(&self.session).is_none() {
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
            let ids = self.changed_entries(update.changes);
            for id in ids {
                self.dirty_entry(&id);
                if let Err(error) =
                    self.documents
                        .observe(&mut self.client, link, &self.session, &id)
                {
                    self.view.status_hint = Some(error.to_string());
                }
            }
            changed = true;
        }
        for failure in effects.failures {
            self.view.status_hint = Some(failure.error.message);
        }

        Ok(changed)
    }

    fn changed_entries(
        &mut self,
        changes: carlos_client::ChangeSet,
    ) -> Vec<carlos_proto::ids::EntryId> {
        if changes.reset || changes.removed_front > 0 || changes.removed_back > 0 {
            self.dirty_from(0);
            self.documents.prune(&self.client, &self.session);
            return self
                .client
                .session(&self.session)
                .into_iter()
                .flat_map(|replica| replica.entries())
                .rev()
                .take(32)
                .map(|entry| entry.id.clone())
                .collect();
        }
        changes.entries
    }

    pub(super) fn event(&mut self, mut link: Option<&mut Transport>, event: Event) -> Result<bool> {
        match event {
            Event::Mouse(mouse) => self.mouse(mouse)?,
            Event::Paste(text) => {
                self.view.input.insert_str(normalize_pasted_text(&text));
            }
            Event::Key(key) if is_key_press_like(key.kind) => {
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(true);
                }
                match self.approval_key(link.as_deref_mut(), key) {
                    Ok(true) => return Ok(false),
                    Err(error) => {
                        self.view.status_hint = Some(error.to_string());
                        return Ok(false);
                    }
                    Ok(false) => {}
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
            }
            _ => {}
        }
        Ok(false)
    }

    fn submit(&mut self, link: Option<&mut Transport>) -> Result<()> {
        let text = self.view.input.lines().join("\n");
        if self.local_command(&text) {
            return Ok(());
        }
        let link = link.ok_or_else(|| anyhow::anyhow!("Disconnected; F5 reconnects"))?;
        let session = self.session.clone();
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
        let outgoing = self
            .client
            .submit(link.connection, |operation| Command::Send {
                session,
                operation,
                text,
                policy: Default::default(),
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

    fn dirty_entry(&mut self, entry: &carlos_proto::ids::EntryId) {
        if let Some(position) = self
            .client
            .session(&self.session)
            .and_then(|replica| replica.entry_position(entry))
        {
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
