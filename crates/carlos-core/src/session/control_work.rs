//! Execute priority controls away from the owner and settle only their original turn or card.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::ids::{SessionId, TurnId};
use carlos_proto::session::Approval;
use serde_json::Value;

use super::approval_state::{ApprovalChoice, ApprovalClaim};
use super::entry_id::EntryId;
use super::state::SessionState;
use crate::backend::{BackendClient, BackendKind};
use crate::backend_commands::{CommandHandle, InterruptIntent};

/// Immutable interrupt or card-answer work whose wait runs outside SessionState.
pub struct ControlWork {
    session: SessionId,
    kind: BackendKind,
    generation: Option<u64>,
    call: ControlCall,
}

pub(super) enum ControlCall {
    QuestionInterrupt {
        claim: Box<ApprovalClaim>,
        params: Value,
        intent: Option<InterruptIntent>,
    },
    NativeAnswer {
        claim: Box<ApprovalClaim>,
        body: Value,
    },
    Interrupt {
        turn: TurnId,
        params: Value,
        intent: InterruptIntent,
        live: Arc<AtomicBool>,
    },
    Answer {
        claim: Box<ApprovalClaim>,
        card: Approval,
        answer: ApprovalAnswer,
    },
}

/// A control result and its exact target, settled once by the session owner.
pub struct ControlCompletion {
    session: SessionId,
    call: ControlCall,
    result: Result<(Option<ApprovalChoice>, Value), ControlFailure>,
}

/// Invalid input can reopen a card; a native write failure remains uncertain.
#[derive(Debug)]
pub enum ControlFailure {
    Invalid(String),
    Retired,
    Native(String),
}

impl ControlWork {
    /// Run the captured control without borrowing its session.
    pub fn execute(self, client: &CommandHandle) -> ControlCompletion {
        let result = if client.kind() != self.kind
            || self
                .generation
                .zip(client.connection_generation())
                .is_some_and(|(expected, actual)| expected != actual)
        {
            Err(ControlFailure::Retired)
        } else {
            let live = match &self.call {
                ControlCall::Interrupt { live, .. } => live.clone(),
                ControlCall::Answer { claim, .. }
                | ControlCall::NativeAnswer { claim, .. }
                | ControlCall::QuestionInterrupt { claim, .. } => claim.live.clone(),
            };
            match client.guarded(crate::dispatch_guard::DispatchGuard::from_flag(live)) {
                Ok(commands) => super::control_execute::execute(&self.call, &commands),
                Err(error) => Err(ControlFailure::Native(error.to_string())),
            }
        };
        ControlCompletion {
            session: self.session,
            call: self.call,
            result,
        }
    }
}

impl SessionState {
    /// Prepare only the exact current public turn without entering the ordinary mutation slot.
    pub fn prepare_engine_interrupt(
        &mut self,
        client: &dyn BackendClient,
        expected: &TurnId,
    ) -> Result<ControlWork> {
        self.refresh_wire_turn();
        let turn = self
            .events
            .current_turn
            .as_mut()
            .filter(|turn| &turn.id == expected)
            .context("interrupt turn is stale")?;
        ensure!(
            !turn.interrupting,
            "interrupt for this turn is already in flight"
        );
        turn.interrupting = true;
        let turn_id = turn.id.clone();
        let native = turn.native.clone();
        let live = turn
            .live
            .get_or_insert_with(|| Arc::new(AtomicBool::new(true)))
            .clone();
        let params = crate::protocol_params::params_turn_interrupt(self.thread_id(), &native);
        Ok(ControlWork {
            session: self.session_id().clone(),
            generation: self.control_generation(client.kind()),
            kind: client.kind(),
            call: ControlCall::Interrupt {
                turn: turn_id,
                live,
                params,
                intent: client.prepare_interrupt(),
            },
        })
    }

    /// Claim an exact live card once while the worker validates its answer.
    pub fn prepare_engine_answer(
        &mut self,
        client: &dyn BackendClient,
        id: EntryId,
        answer: ApprovalAnswer,
    ) -> Result<ControlWork> {
        let kind = client.kind();
        if kind == BackendKind::Claude && matches!(answer, ApprovalAnswer::InterruptQuery) {
            self.refresh_wire_turn();
        }
        let pending = self
            .approval()
            .live()
            .find(|card| card.entry_id() == id)
            .context("approval card is retired or foreign")?;
        let active = self
            .approval()
            .pending()
            .is_some_and(|card| card.entry_id() == id);
        let card = super::event_approval::export_card(
            pending,
            active,
            self.approval_blocks(pending),
            self.approval_browser_completed(pending),
            self.approval_caller(pending),
        );
        ensure!(!card.submitted, "approval already has a submitted answer");
        let interrupt = self.question_interrupt(kind, &card, &answer)?;
        let claim = self
            .claim_live_approval(id)
            .context("approval already has a submitted answer")?;
        Ok(ControlWork {
            session: self.session_id().clone(),
            generation: self.control_generation(kind),
            kind,
            call: if let Some(params) = interrupt {
                ControlCall::QuestionInterrupt {
                    claim: Box::new(claim),
                    params,
                    intent: (kind == BackendKind::Claude).then(|| client.prepare_interrupt()),
                }
            } else {
                ControlCall::Answer {
                    claim: Box::new(claim),
                    card,
                    answer,
                }
            },
        })
    }

    pub(in crate::session) fn prepare_checked_native_answer(
        &self,
        kind: BackendKind,
        id: EntryId,
        body: Value,
    ) -> Option<ControlWork> {
        let claim = self
            .approval()
            .live()
            .find(|card| card.entry_id() == id)?
            .submitted_claim()?;
        Some(ControlWork {
            session: self.session_id().clone(),
            kind,
            generation: self.control_generation(kind),
            call: ControlCall::NativeAnswer {
                claim: Box::new(claim),
                body,
            },
        })
    }

    /// Settle the original control without retiring a newer turn or another client's card.
    pub fn finish_engine_control(
        &mut self,
        client: &dyn BackendClient,
        completion: ControlCompletion,
    ) -> Result<(), ControlFailure> {
        if &completion.session != self.session_id() {
            return Err(ControlFailure::Retired);
        }
        match &completion.call {
            ControlCall::Interrupt { turn, .. } => {
                if let Some(current) = self
                    .events
                    .current_turn
                    .as_mut()
                    .filter(|current| &current.id == turn)
                {
                    current.interrupting = false;
                    if let Ok((_, response)) = &completion.result {
                        self.record_interrupt_ack(response, client.kind());
                    }
                }
            }
            ControlCall::QuestionInterrupt { claim, .. } => {
                if claim.live.load(Ordering::SeqCst) {
                    self.reopen_claimed_approval(claim);
                    if completion.result.is_ok() {
                        self.edit_codex_callback(&claim.facts.request_id);
                        if let Some(callback) =
                            self.codex_callbacks.get_mut(&claim.facts.request_id)
                        {
                            callback.interrupting = true;
                        }
                    }
                }
            }
            ControlCall::Answer { claim, .. } | ControlCall::NativeAnswer { claim, .. } => {
                match &completion.result {
                    Ok((Some(choice), _)) if claim.live.load(Ordering::SeqCst) => {
                        self.finish_claimed_answer(claim, *choice)
                    }
                    Err(ControlFailure::Invalid(_)) => self.reopen_claimed_approval(claim),
                    _ => {}
                }
            }
        }
        match completion.call {
            ControlCall::Interrupt { intent, .. }
            | ControlCall::QuestionInterrupt {
                intent: Some(intent),
                ..
            } => client.finish_interrupt(intent, completion.result.is_ok()),
            _ => {}
        }
        completion.result.map(|_| ())
    }

    fn record_interrupt_ack(&mut self, response: &Value, kind: BackendKind) {
        if let Some(ids) = response["result"]["still_queued"]
            .as_array()
            .filter(|ids| !ids.is_empty())
        {
            self.append_message(super::models::Role::System, format!("Main-turn interrupt requested; {} accepted queued prompt(s) remain with Claude and will still run", ids.len()));
        }
        if kind != BackendKind::Claude {
            self.append_turn_interrupted_marker();
        }
        self.set_status("interrupt requested");
    }

    fn control_generation(&self, kind: BackendKind) -> Option<u64> {
        match kind {
            BackendKind::Codex => Some(self.codex_connection().generation()),
            BackendKind::Claude => Some(self.claude().generation),
            BackendKind::Grok => None,
        }
    }

    fn finish_claimed_answer(&mut self, claim: &ApprovalClaim, choice: ApprovalChoice) {
        self.record_approval_decision(claim.card, choice);
        self.finish_claimed_claude_browser(&claim.facts, choice);
        if self
            .codex_callbacks()
            .get(&claim.facts.request_id)
            .is_some()
        {
            self.retire_codex_callback(&claim.facts.request_id);
        } else if self
            .approval()
            .pending()
            .is_some_and(|card| card.entry_id() == claim.card)
        {
            self.clear_pending_session_approval();
        } else {
            let _ = self.retire_approval(claim.card);
        }
        self.set_status("answer sent");
    }
}

impl std::fmt::Display for ControlFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) | Self::Native(message) => f.write_str(message),
            Self::Retired => f.write_str("control target retired; outcome must be checked"),
        }
    }
}
impl std::error::Error for ControlFailure {}
