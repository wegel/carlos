//! Translate explicit typed root-goal commands and settle their original native request outcomes.

use std::time::Instant;

use carlos_proto::commands::RpcError;
use carlos_proto::goals::GoalChange;
use carlos_proto::ids::GoalRevision;
use carlos_proto::submissions::{ClientOperation, OperationState};
use serde_json::{json, Value};

use super::owner::{failure, Owner};
use super::target::Target;
use super::{EngineCommand, EngineReply};
use crate::backend::BackendKind;
use crate::session::codex::goals::{self, GoalUpdate};

pub(super) struct PendingGoal {
    operation: ClientOperation,
    target: Target,
}

impl Owner {
    pub(super) fn read_goal(&mut self, refresh: bool) -> Result<EngineReply, RpcError> {
        self.require_goal_backend()?;
        if refresh && !self.native.closed {
            self.session.refresh_codex_goal();
            let scope = self.session.codex_scope();
            goals::tick(
                self.client.as_ref(),
                &mut self.session,
                scope,
                Instant::now(),
                false,
            );
        }
        self.publish();
        Ok(EngineReply::Goal(carlos_proto::goals::GoalSnapshot {
            session: self.session.session_id().clone(),
            seq: self.ring.sequence(),
            goal: self.session.export_goal().expect("required Codex goal"),
        }))
    }

    pub(super) fn change_goal(&mut self, command: EngineCommand) {
        let (operation, expected, change) = match command {
            EngineCommand::GoalSet {
                operation,
                expected,
                change,
            } => (operation, expected, Some(change)),
            EngineCommand::GoalClear {
                operation,
                expected,
            } => (operation, expected, None),
            _ => unreachable!("typed goal mutation"),
        };
        let prepared = self.prepare_goal(&expected, change);
        match prepared {
            Err(error) => self
                .results
                .update(&operation, super::dispatch::rejected(error.message)),
            Ok((method, params)) => {
                let commands = match self.client.capture_commands() {
                    Ok(commands) => commands,
                    Err(error) => {
                        self.results
                            .update(&operation, super::dispatch::rejected(error.to_string()));
                        self.notice();
                        return;
                    }
                };
                let scope = self.session.codex_scope();
                let target = Target::capture(&self.session, self.client.kind());
                goals::mutate_owned(
                    &commands,
                    &mut self.session,
                    scope,
                    method,
                    params,
                    operation.clone(),
                );
                if let Some(request) = &mut self.session.codex_goals.mutation {
                    debug_assert_eq!(request.operation.as_ref(), Some(&operation));
                    self.goal = Some(PendingGoal { operation, target });
                } else {
                    self.results.update(
                        &operation,
                        super::dispatch::rejected(
                            self.session
                                .codex_goals
                                .error
                                .as_deref()
                                .unwrap_or("goal request was not admitted"),
                        ),
                    );
                }
            }
        }
        self.notice();
    }

    pub(super) fn finish_goal(&mut self, update: GoalUpdate) {
        if let Some((operation, state)) = update.outcome {
            if self
                .goal
                .as_ref()
                .is_some_and(|pending| pending.operation == operation)
            {
                let pending = self.goal.take().expect("matched goal");
                let state = if pending.target.matches(&self.session, self.client.kind()) {
                    state
                } else {
                    uncertain("native goal acknowledgement belongs to a retired root")
                };
                self.results.update(&operation, state);
                self.notice();
            }
        }
    }

    pub(super) fn retire_goal_scope(&mut self) {
        if self.goal.as_ref().is_some_and(|pending| {
            self.native.closed || !pending.target.matches(&self.session, self.client.kind())
        }) {
            let pending = self.goal.take().expect("retired goal");
            self.results.update(
                &pending.operation,
                uncertain("native goal connection changed before its outcome was confirmed"),
            );
            self.notice();
        }
    }

    fn require_goal_backend(&self) -> Result<(), RpcError> {
        if self.client.kind() != BackendKind::Codex || !self.session.codex_scope().enabled() {
            return Err(failure("this backend does not support native Codex goals"));
        }
        Ok(())
    }

    fn prepare_goal(
        &self,
        expected: &GoalRevision,
        change: Option<GoalChange>,
    ) -> Result<(&'static str, Value), RpcError> {
        self.require_goal_backend()?;
        if expected != &self.session.goal_revision() {
            return Err(failure(
                "goal changed; read the current goal before submitting this draft",
            ));
        }
        let active = change
            .as_ref()
            .is_some_and(|change| change.status == carlos_proto::goals::GoalStatus::Active);
        goals::can_mutate(&self.session, self.session.codex_scope(), active).map_err(failure)?;
        let mut params = json!({"threadId":self.session.thread_id(),"origin":"user"});
        let Some(change) = change else {
            return Ok(("thread/goal/clear", params));
        };
        if change.objective.as_ref().is_some_and(|text| {
            text.trim().is_empty() || text.len() > carlos_proto::goals::MAX_OBJECTIVE_BYTES
        }) || change
            .token_budget
            .is_some_and(|budget| budget.get() == 0 || budget.get() > i64::MAX as u64)
        {
            return Err(failure(
                "goal needs a nonempty objective up to 32 KiB and a positive signed 64-bit budget",
            ));
        }
        if change.objective.is_none() && self.session.codex_goals.goal.is_null() {
            return Err(failure(
                "no native goal exists; supply an objective for a new paused or active goal",
            ));
        }
        params["status"] = json!(change.status.as_str());
        if let Some(objective) = change.objective {
            params["objective"] = json!(objective);
        }
        if let Some(budget) = change.token_budget {
            params["tokenBudget"] = json!(budget.get());
        }
        if params.to_string().len() > carlos_proto::goals::MAX_GOAL_BYTES - 1024 {
            return Err(failure(
                "escaped goal text exceeds the native 128 KiB budget; nothing sent",
            ));
        }
        Ok(("thread/goal/set", params))
    }
}

fn uncertain(message: &str) -> OperationState {
    OperationState::Uncertain {
        message: message.into(),
        recovery: None,
    }
}
