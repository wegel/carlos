//! Resolve stable prompt choices and fork before sending edited text.

use std::time::Duration;

use crate::backend::{BackendClient, BackendKind, PromptOrigin, RewindForkRequest};

use super::claude::receive_update::ClaudeEffect;
use super::claude::ClaudePhase;
use super::entry_id::EntryId;
use super::models::{EntryDraft, Role};
use super::prompt_commands::{PromptContext, PromptOptions, TurnUpdate};
use super::state::SessionState;

impl SessionState {
    /// Resolve backend positions from the selected id, never from a view's cached count.
    pub(crate) fn submit_rewind_text(
        &mut self,
        client: &dyn BackendClient,
        selected: EntryId,
        text: String,
        context: &PromptContext<'_>,
        queue_recovery: bool,
    ) -> TurnUpdate {
        let mut update = TurnUpdate::default();
        if text.trim().is_empty() || self.reject_rewind(client, queue_recovery, &mut update) {
            return update;
        }
        let Some((index, entry, turn)) = self.rewind_anchor(selected) else {
            update.hint =
                Some("The selected prompt is no longer available; choose it again".into());
            return update;
        };
        self.rewind_from_anchor(client, (index, entry, turn), text, context, update)
    }

    fn rewind_from_anchor(
        &mut self,
        client: &dyn BackendClient,
        (index, entry, turn): (usize, Option<EntryId>, Option<String>),
        text: String,
        context: &PromptContext<'_>,
        mut update: TurnUpdate,
    ) -> TurnUpdate {
        let drop_turns = self.input_history().history_len().saturating_sub(index);
        let options = PromptOptions {
            record_history: true,
            force_start: false,
            origin: PromptOrigin::Human,
        };
        if drop_turns == 0 && client.kind() != BackendKind::Claude {
            return self.submit_turn_text(client, text, options, context);
        }
        let prefix =
            (client.kind() == BackendKind::Grok).then(|| self.grok_rewind_prefix(index, entry));
        let request = self.rewind_request(client.kind(), index, entry, turn);
        let response =
            match client.fork_from_rewind(self.thread_id(), request, Duration::from_secs(20)) {
                Ok(response) => response,
                Err(error) => {
                    self.rewind_failed(client, &error.to_string(), &mut update);
                    return update;
                }
            };
        if client.kind() == BackendKind::Claude {
            self.begin_claude_edit(text);
            update.changes.effects.push(ClaudeEffect::EditFailed(false));
            self.retire_claude_queue(&mut update.changes);
            update.changes.effects.push(ClaudeEffect::FollowBottom);
            return update;
        }
        let adopted = if let Some(prefix) = prefix {
            self.adopt_grok_fork(&response, prefix)
        } else {
            self.adopt_codex_fork(&response)
        };
        match adopted {
            Err(error) => {
                self.set_status(format!("fork failed: {error}"));
                return update;
            }
            Ok(changes) => update.merge(changes),
        }
        self.submit_edited_prompt(client, text, options, context, &mut update);
        update
    }

    fn rewind_request(
        &self,
        kind: BackendKind,
        index: usize,
        entry: Option<EntryId>,
        turn: Option<String>,
    ) -> RewindForkRequest {
        RewindForkRequest {
            keep_turns: index,
            drop_turns: self.input_history().history_len().saturating_sub(index),
            before_turn_id: turn,
            before_user_item: (kind == BackendKind::Codex)
                .then(|| entry.and_then(|id| self.input_history().native_request(index, id)))
                .flatten(),
        }
    }

    fn rewind_anchor(&self, selected: EntryId) -> Option<(usize, Option<EntryId>, Option<String>)> {
        if let Some(prompt) = self
            .claude_history()
            .unknown()
            .iter()
            .find(|p| p.entry_id() == selected)
        {
            return Some((usize::MAX, Some(selected), Some(prompt.uuid().into())));
        }
        let index = self.input_history().prompt_index(selected)?;
        let entry = self.input_history().entry_id(index);
        Some((
            index,
            entry,
            entry.and_then(|id| self.input_history().turn_id(id).map(str::to_string)),
        ))
    }

    fn reject_rewind(
        &mut self,
        client: &dyn BackendClient,
        recovery: bool,
        update: &mut TurnUpdate,
    ) -> bool {
        if client.kind() == BackendKind::Codex && (self.codex_queue().pending() || recovery) {
            update.changes.items.track(self.append_message(Role::System,
                "Finish or recover the Codex queue request before editing earlier history. This edited prompt is preserved."));
            return true;
        }
        if client.kind() == BackendKind::Claude && self.claude().phase() != ClaudePhase::Ready {
            if self.claude_edit_progress().is_none() {
                self.unavailable_claude_edit(
                    &mut update.changes,
                    "Claude must be ready before editing an earlier prompt",
                );
            }
            return true;
        }
        false
    }

    fn grok_rewind_prefix(&self, index: usize, entry: Option<EntryId>) -> Vec<EntryDraft> {
        let cut = entry
            .and_then(|id| self.messages().position(id))
            .or_else(|| {
                self.messages()
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m.role == Role::User)
                    .nth(index)
                    .map(|(index, _)| index)
            })
            .unwrap_or(self.messages().len())
            .min(self.messages().len());
        self.messages()[..cut]
            .iter()
            .cloned()
            .map(Into::into)
            .collect()
    }

    fn rewind_failed(&mut self, client: &dyn BackendClient, error: &str, update: &mut TurnUpdate) {
        self.set_status(format!("fork failed: {error}"));
        update
            .changes
            .items
            .track(self.append_message(Role::System, format!("Could not edit message: {error}")));
        if client.kind() == BackendKind::Codex {
            update.changes.effects.push(ClaudeEffect::FollowBottom);
        }
    }

    fn submit_edited_prompt(
        &mut self,
        client: &dyn BackendClient,
        text: String,
        options: PromptOptions,
        context: &PromptContext<'_>,
        update: &mut TurnUpdate,
    ) {
        update.changes.effects.push(ClaudeEffect::ClearRewind);
        update.clear_selection = true;
        self.ralph_mark_user_turn_submitted();
        if self.start_prompt(
            client,
            text.clone(),
            PromptOptions {
                force_start: true,
                ..options
            },
            context,
            update,
        ) {
            if client.kind() == BackendKind::Codex {
                update.changes.items.reset_navigation |= self.record_prompt_history(&text, None);
            }
            update.changes.effects.push(ClaudeEffect::ClearInput);
        }
    }
}
