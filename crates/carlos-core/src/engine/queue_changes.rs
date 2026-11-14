//! Translate revision-checked queue changes and retain private results through native waits.

use carlos_proto::commands::RpcError;
use carlos_proto::ids::QueueRevision;
use carlos_proto::queue::{QueueChange, QueueDirection};
use carlos_proto::submissions::{ClientOperation, OperationState, SubmittedText};
use serde_json::{json, Value};

use super::owner::{failure, Owner};
use super::target::Target;
use crate::session::codex::queue::{self, QueueUpdate};
use crate::session::codex::queue_state::editable_text;

pub(super) struct PendingQueue {
    operation: ClientOperation,
    target: Target,
    draft: Option<SubmittedText>,
}

impl Owner {
    pub(super) fn change_queue(
        &mut self,
        operation: ClientOperation,
        expected: QueueRevision,
        change: QueueChange,
    ) {
        let draft = change.text().map(|text| SubmittedText {
            operation: Some(operation.clone()),
            text: text.into(),
        });
        let prepared = self
            .prepare_queue(&operation, &expected, change)
            .and_then(|request| {
                self.client
                    .capture_commands()
                    .map(|commands| (request, commands))
                    .map_err(|error| failure(error.to_string()))
            });
        let ((method, params), commands) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.reject_queue(&operation, draft, error.message);
                return;
            }
        };
        let scope = self.session.codex_scope();
        let target = Target::capture(&self.session, self.client.kind());
        if queue::mutate_owned(
            &commands,
            &mut self.session,
            scope,
            operation.clone(),
            method,
            params,
            None,
        ) {
            self.queue = Some(PendingQueue {
                operation,
                target,
                draft,
            });
        } else {
            let message = self
                .session
                .codex_queue
                .error
                .clone()
                .unwrap_or_else(|| "Queue change was not admitted".into());
            self.reject_queue(&operation, draft, message);
        }
        self.notice();
    }

    fn reject_queue(
        &mut self,
        operation: &ClientOperation,
        recovery: Option<SubmittedText>,
        message: String,
    ) {
        self.results
            .update(operation, OperationState::Rejected { message, recovery });
        self.notice();
    }

    pub(super) fn finish_queue(&mut self, update: QueueUpdate) {
        let Some((operation, mut state)) = update.outcome else {
            return;
        };
        if !self
            .queue
            .as_ref()
            .is_some_and(|pending| pending.operation == operation)
        {
            return;
        }
        let pending = self.queue.take().expect("matching queue operation");
        if !pending.target.matches(&self.session, self.client.kind()) {
            state = uncertain("native queue acknowledgement belongs to a retired root");
        }
        with_recovery(&mut state, pending.draft);
        self.results.update(&operation, state);
        self.notice();
    }

    pub(super) fn retire_queue_scope(&mut self) {
        if self.queue.as_ref().is_some_and(|pending| {
            self.native.closed || !pending.target.matches(&self.session, self.client.kind())
        }) {
            let pending = self.queue.take().expect("retired queue operation");
            let mut state =
                uncertain("native queue connection changed before its outcome was confirmed");
            with_recovery(&mut state, pending.draft);
            self.results.update(&pending.operation, state);
            self.notice();
        }
    }

    fn prepare_queue(
        &self,
        operation: &ClientOperation,
        expected: &QueueRevision,
        change: QueueChange,
    ) -> Result<(&'static str, Value), RpcError> {
        self.require_queue_backend()?;
        self.require_queue_revision(expected)?;
        if self.native.closed
            || self.session.codex_queue.pending()
            || self.session.codex_queue.exhausted
        {
            return Err(failure(
                "queue change is pending, exhausted or Codex is closed",
            ));
        }
        if change.text().is_some_and(|text| {
            text.trim().is_empty() || text.len() > carlos_proto::queue::MAX_TEXT_BYTES
        }) {
            return Err(failure("queued text must be nonempty and at most 512 KiB"));
        }
        let (method, params) = self.queue_params(operation, change)?;
        if params.to_string().len() > crate::protocol_writer::MAX_FRAME_BYTES - 1024 {
            return Err(failure(
                "escaped queue request exceeds the native frame budget; nothing sent",
            ));
        }
        Ok((method, params))
    }
    fn queue_params(
        &self,
        operation: &ClientOperation,
        change: QueueChange,
    ) -> Result<(&'static str, Value), RpcError> {
        let mut params = json!({"threadId":self.session.thread_id()});
        let method = match change {
            QueueChange::Add { text } => {
                params["clientUserMessageId"] =
                    json!(format!("carlos-{}-{}", operation.client, operation.id));
                params["input"] = input(text);
                "thread/queue/add"
            }
            QueueChange::Edit { entry, text } => {
                let (_, native) = self
                    .session
                    .queue_entry(&entry)
                    .ok_or_else(|| failure("retired or foreign queue entry"))?;
                editable_text(native).ok_or_else(|| failure("edit unavailable: native input has attachments, spans or unsupported fields"))?;
                params["queuedSubmissionId"] = native["id"].clone();
                params["input"] = input(text);
                "thread/queue/update"
            }
            QueueChange::Delete { ref entry } | QueueChange::Start { ref entry } => {
                let index = self.queue_index(entry)?;
                params["queuedSubmissionId"] =
                    self.session.codex_queue.entries[index]["id"].clone();
                if matches!(change, QueueChange::Start { .. }) {
                    "thread/queue/start"
                } else {
                    "thread/queue/delete"
                }
            }
            QueueChange::Move { entry, direction } => {
                let index = self.queue_index(&entry)?;
                let entries = &self.session.codex_queue.entries;
                let next = match direction {
                    QueueDirection::Up => index.saturating_sub(1),
                    QueueDirection::Down => (index + 1).min(entries.len() - 1),
                };
                let mut ids: Vec<_> = entries.iter().map(|entry| entry["id"].clone()).collect();
                ids.swap(index, next);
                params["queuedSubmissionIds"] = json!(ids);
                "thread/queue/reorder"
            }
        };
        Ok((method, params))
    }
}

fn input(text: String) -> Value {
    json!([{"type":"text", "text":text, "text_elements":[]}])
}

fn uncertain(message: &str) -> OperationState {
    OperationState::Uncertain {
        message: message.into(),
        recovery: None,
    }
}

fn with_recovery(state: &mut OperationState, draft: Option<SubmittedText>) {
    if let OperationState::Rejected { recovery, .. } | OperationState::Uncertain { recovery, .. } =
        state
    {
        *recovery = draft;
    }
}
