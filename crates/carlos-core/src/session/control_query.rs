//! Validate an explicit interrupt against the original native callback and offered cancel choice.

use anyhow::{ensure, Context, Result};
use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::{Approval, Decision};
use serde_json::Value;

use super::approval_state::ApprovalRequestKind;
use super::state::SessionState;
use crate::backend::BackendKind;

impl SessionState {
    pub(super) fn question_interrupt(
        &self,
        kind: BackendKind,
        card: &Approval,
        answer: &ApprovalAnswer,
    ) -> Result<Option<Value>> {
        let explicit = matches!(answer, ApprovalAnswer::InterruptQuery);
        if !explicit
            && !matches!(
                answer,
                ApprovalAnswer::Decision {
                    decision: Decision::Cancel
                }
            )
        {
            return Ok(None);
        }
        let pending = self
            .approval()
            .live()
            .find(|pending| pending.entry_id().to_wire() == card.id)
            .context("original question retired")?;
        if !explicit && pending.facts().kind != ApprovalRequestKind::CodexQuestion {
            return Ok(None);
        }
        ensure!(
            card.choices.contains(&Decision::Cancel),
            "original request does not offer interrupting"
        );
        if kind == BackendKind::Claude {
            ensure!(
                explicit && pending.facts().method == "claude/native",
                "original native helper request is unavailable"
            );
            ensure!(
                self.events.current_turn.is_none(),
                "interrupt the accepted public turn instead of guessing a helper target"
            );
            return Ok(Some(serde_json::json!({})));
        }
        ensure!(
            kind == BackendKind::Codex,
            "this backend has no card-scoped query interrupt"
        );
        let callback = self
            .codex_callbacks()
            .get(&pending.facts().request_id)
            .context("original callback retired")?;
        ensure!(
            !callback.interrupting,
            "interrupt for this original query is already pending"
        );
        super::control_answers::question_interrupt_params(callback.params())
            .map(Some)
            .context("original question has no native interrupt target")
    }
}
