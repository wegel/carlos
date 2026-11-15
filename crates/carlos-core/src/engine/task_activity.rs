//! Settle addressed child readers without polling native history or blocking control workers.

use carlos_proto::commands::RpcError;

use super::mailbox::Envelope;
use super::owner::{failure, Owner};
use super::{EngineCommand, EngineReply};
use crate::backend::BackendKind;
use crate::session::codex::panel;
use crate::session::task_activity::ActivityCommand;
use crate::session::task_activity_reads::activity_key;

impl Owner {
    pub(super) fn read_task_activity(
        &mut self,
        command: ActivityCommand,
    ) -> Result<EngineReply, RpcError> {
        if self.client.kind() != BackendKind::Codex || !self.session.codex_scope().enabled() {
            return Err(failure("this backend has no native child activity reader"));
        }
        match command {
            ActivityCommand::Open {
                reader,
                task,
                expected,
                window,
            } => {
                let context =
                    self.session
                        .open_task_activity(reader, task, &expected, window.as_ref())?;
                self.tick_child_reads();
                self.session
                    .task_activity_header(&context)
                    .map(EngineReply::TaskActivity)
            }
            ActivityCommand::Read { context, since } => {
                let header = self.session.task_activity_header(&context)?;
                if since.is_some_and(|since| since > header.change) {
                    return Err(failure(
                        "child change cursor is ahead of its original reader",
                    ));
                }
                Ok(EngineReply::TaskActivity(header))
            }
            ActivityCommand::Page {
                context,
                expected,
                after,
                limit,
            } => self
                .session
                .task_activity_page(&context, &expected, after.as_ref(), limit)
                .map(EngineReply::TaskActivityPage),
            ActivityCommand::Advance { context, older } => {
                self.session.activity_for_context(&context)?;
                self.session
                    .start_activity_read(&activity_key(&context.activity), older)
                    .map_err(|error| failure(&error))?;
                self.tick_child_reads();
                self.session
                    .task_activity_header(&context)
                    .map(EngineReply::TaskActivity)
            }
            ActivityCommand::Close { reader, activity } => {
                self.session
                    .close_task_activities(&reader, activity.as_ref());
                Ok(EngineReply::TaskActivityClosed)
            }
        }
    }

    fn tick_child_reads(&mut self) {
        let scope = self.task_read_scope();
        panel::tick(
            self.client.as_ref(),
            &mut self.session,
            scope,
            &mut Default::default(),
        );
    }

    pub(super) fn defer_task_activity_read(&mut self, envelope: Envelope) -> Option<Envelope> {
        let EngineCommand::TaskActivity(ActivityCommand::Read {
            context,
            since: Some(since),
        }) = &envelope.command
        else {
            return Some(envelope);
        };
        if !self
            .session
            .task_activity_header(context)
            .is_ok_and(|header| header.change == *since && !header.closed)
        {
            return Some(envelope);
        }
        let duplicate = self.task_activity_reads.iter().any(|pending| matches!(&pending.command,
            EngineCommand::TaskActivity(ActivityCommand::Read { context: previous, .. }) if previous.activity == context.activity));
        if duplicate || self.task_activity_reads.len() >= 8 {
            let _ = envelope.reply.try_send(EngineReply::Failure(failure(
                "this child reader already has an admitted change read",
            )));
        } else {
            self.task_activity_reads.push(envelope);
        }
        None
    }

    pub(super) fn finish_task_activity_reads(&mut self) {
        self.session.expire_task_activities();
        for envelope in std::mem::take(&mut self.task_activity_reads) {
            let EngineCommand::TaskActivity(ActivityCommand::Read { context, since }) =
                &envelope.command
            else {
                unreachable!("deferred child header")
            };
            match self.session.task_activity_header(context) {
                Ok(header) if Some(header.change) == *since && !header.closed => {
                    self.task_activity_reads.push(envelope)
                }
                result => {
                    let reply = result
                        .map(EngineReply::TaskActivity)
                        .unwrap_or_else(EngineReply::Failure);
                    let _ = envelope.reply.try_send(reply);
                    let _ = self.channels.notice.try_send(());
                }
            }
        }
    }
}
