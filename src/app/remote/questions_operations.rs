//! Keep one exact callback answer and its private outcome separate from every other draft.

use anyhow::{ensure, Result};
use carlos_proto::commands::{ApprovalAnswer, Command};
use carlos_proto::ids::EntryId;
use carlos_proto::session::{Approval, Backend, Decision};
use carlos_proto::submissions::{ClientOperation, OperationState};

use super::questions::{fingerprint, AnswerTarget};
use super::transport::Transport;
use super::Device;

impl Device {
    pub(super) fn send_question_answer(
        &mut self,
        link: Option<&mut Transport>,
        id: EntryId,
        answer: ApprovalAnswer,
    ) -> Result<()> {
        let link = link.ok_or_else(|| {
            anyhow::anyhow!("Disconnected; F5 reconnects. Your answer draft is kept")
        })?;
        let target = self.prepare_question_answer(link, &id, &answer)?;
        let interrupt = target.interrupt;
        let session = self.session.clone();
        let approval = id.clone();
        let mut submitted = None;
        let outgoing = self.client.submit(link.connection, |operation| {
            submitted = Some(operation.clone());
            Command::Approve {
                session,
                operation,
                approval,
                answer,
            }
        })?;
        self.questions
            .operations
            .insert(submitted.expect("allocated private answer"), target);
        if let Some(draft) = self.questions.drafts.get_mut(&id) {
            draft.editor.error = None;
            draft.editor.notice = Some(
                if interrupt {
                    "Interrupt query pending; original question kept"
                } else {
                    "Answer pending for this original request"
                }
                .into(),
            );
        }
        link.send(outgoing)
    }

    fn question_card(&self, id: &EntryId) -> Result<&Approval> {
        self.client
            .session(&self.session)
            .and_then(|replica| replica.approvals().iter().find(|card| &card.id == id))
            .ok_or_else(|| {
                anyhow::anyhow!("The original request retired; no answer was redirected")
            })
    }

    fn question_answer_pending(&self, id: &EntryId) -> bool {
        self.questions.operations.iter().any(|(operation, target)| {
            &target.card == id
                && (target.finished
                    || self
                        .client
                        .operations()
                        .get(operation)
                        .is_some_and(|record| {
                            matches!(
                                record.state,
                                OperationState::Pending | OperationState::Queued
                            )
                        }))
        })
    }

    fn prepare_question_answer(
        &self,
        link: &Transport,
        id: &EntryId,
        answer: &ApprovalAnswer,
    ) -> Result<AnswerTarget> {
        ensure!(
            self.client
                .session_has_current_snapshot(link.connection, &self.session)
                && self.question_facts_are_current(),
            "Wait for accepted session facts before answering; your draft is kept"
        );
        let card = self.question_card(id)?;
        ensure!(
            !card.submitted && !self.question_answer_pending(id),
            "This original request already has a pending answer"
        );
        let original = fingerprint(card);
        let interrupt = matches!(*answer, ApprovalAnswer::InterruptQuery)
            || self.task_backend() == Some(Backend::Codex)
                && card.kind == "question"
                && matches!(
                    *answer,
                    ApprovalAnswer::Decision {
                        decision: Decision::Cancel
                    }
                );
        ensure!(
            self.questions
                .drafts
                .get(id)
                .is_none_or(|draft| draft.fingerprint == original),
            "The original question changed; F4 opens a fresh editor"
        );
        if let ApprovalAnswer::Decision { decision } = answer {
            ensure!(
                card.choices.contains(decision),
                "The original request does not offer this choice"
            );
        }
        Ok(AnswerTarget {
            card: id.clone(),
            fingerprint: original,
            finished: false,
            interrupt,
        })
    }

    pub(super) fn apply_question_operation(&mut self, id: &ClientOperation) -> bool {
        let Some(target) = self.questions.operations.get(id) else {
            return false;
        };
        let Some(record) = self.client.operations().get(id) else {
            return true;
        };
        let message = match &record.state {
            OperationState::Pending | OperationState::Queued => return true,
            OperationState::Accepted { .. } if target.interrupt => {
                "Interrupt query requested; question remains pending".into()
            }
            OperationState::Accepted { .. } => {
                "Answer accepted; waiting for the original native request to retire".to_owned()
            }
            OperationState::Rejected { message, .. } => format!("Answer rejected: {message}"),
            OperationState::Uncertain { message, .. } => {
                format!("Answer outcome uncertain: {message}; no automatic retry")
            }
        };
        let accepted = matches!(record.state, OperationState::Accepted { .. });
        let uncertain = matches!(record.state, OperationState::Uncertain { .. });
        if let Some(draft) = self
            .questions
            .drafts
            .get_mut(&target.card)
            .filter(|draft| draft.fingerprint == target.fingerprint)
        {
            if accepted {
                draft.editor.notice = Some(message.clone())
            } else {
                draft.editor.error = Some(message.clone());
                draft.editor.notice = None;
            }
        }
        self.view.status_hint = Some(message);
        if accepted && !target.interrupt {
            self.questions
                .operations
                .get_mut(id)
                .expect("original answer")
                .finished = true;
        } else if !uncertain {
            self.questions.operations.remove(id);
        }
        if !uncertain {
            self.client.forget_operation(id);
        }
        true
    }
}
