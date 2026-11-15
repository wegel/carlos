//! Resolve stable prompt choices and fork before sending edited text.

use crate::backend::{BackendKind, PromptOrigin, RewindForkRequest};

use super::claude::ClaudePhase;
use super::command_work::{
    BackendCall, CommandError, Finish, PreparedCommand, PreparedStatus, PromptSubmission,
};
use super::entry_id::EntryId;
use super::models::{EntryDraft, Role};
use super::prompt_commands::{PromptContext, PromptOptions, TurnUpdate};
use super::state::SessionState;

impl SessionState {
    /// Resolve backend positions from the selected id, never from a view's cached count.
    pub fn prepare_rewind_text(
        &mut self,
        kind: BackendKind,
        selected: EntryId,
        text: String,
        context: &PromptContext,
        queue_recovery: bool,
    ) -> Result<PreparedCommand, CommandError> {
        self.command_idle()?;
        let mut update = TurnUpdate::default();
        if text.trim().is_empty() || self.reject_rewind(kind, queue_recovery, &mut update) {
            return Ok(PreparedCommand {
                update,
                status: PreparedStatus::Failed(
                    "prompt is empty or session cannot edit history now".into(),
                ),
                ..Default::default()
            });
        }
        let Some((index, entry, turn)) = self.rewind_anchor(selected) else {
            return Ok(PreparedCommand {
                update,
                status: PreparedStatus::Failed("selected prompt is retired or not editable".into()),
                ..Default::default()
            });
        };
        self.rewind_from_anchor(kind, (index, entry, turn), text, context, update)
    }

    fn rewind_from_anchor(
        &mut self,
        kind: BackendKind,
        (index, entry, turn): (usize, Option<EntryId>, Option<String>),
        text: String,
        context: &PromptContext,
        update: TurnUpdate,
    ) -> Result<PreparedCommand, CommandError> {
        let drop_turns = self.input_history().history_len().saturating_sub(index);
        let options = PromptOptions {
            record_history: true,
            force_start: false,
            origin: PromptOrigin::Human,
        };
        if drop_turns == 0 && kind != BackendKind::Claude {
            return self.prepare_turn_text(kind, text, options, context);
        }
        let prefix = (kind == BackendKind::Grok).then(|| self.grok_rewind_prefix(index, entry));
        let request = self.rewind_request(kind, index, entry, turn);
        let submission = PromptSubmission {
            kind,
            text,
            context: context.clone(),
            options,
            steer: false,
        };
        let context = if kind == BackendKind::Claude {
            self.begin_claude_edit_for(
                submission.text.clone(),
                submission.context.operation.clone(),
            );
            self.claude_fork_context()
        } else {
            Default::default()
        };
        let call = BackendCall::Fork {
            thread: self.thread_id().clone(),
            request,
            context,
        };
        let work = self.backend_work(kind, call, Finish::Rewind { submission, prefix }, false);
        Ok(PreparedCommand {
            update,
            work: Some(work),
            ..Default::default()
        })
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
        kind: BackendKind,
        recovery: bool,
        update: &mut TurnUpdate,
    ) -> bool {
        if kind == BackendKind::Codex && (self.codex_queue().pending() || recovery) {
            self.append_message(Role::System,
                "Finish or recover the Codex queue request before editing earlier history. This edited prompt is preserved.");
            return true;
        }
        if kind == BackendKind::Claude && self.claude().phase() != ClaudePhase::Ready {
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
}
