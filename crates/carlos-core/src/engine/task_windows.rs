//! Retain admitted window reads until their original bounded native snapshot settles.

use carlos_proto::commands::RpcError;
use carlos_proto::task_windows::MAX_TASK_WINDOWS;

use super::mailbox::Envelope;
use super::owner::{failure, Owner};
use super::{EngineCommand, EngineReply};
use crate::backend::BackendKind;
use crate::session::codex::panel;
use crate::session::task_windows::WindowCommand;

impl Owner {
    pub(super) fn read_task_window(
        &mut self,
        command: WindowCommand,
    ) -> Result<EngineReply, RpcError> {
        if self.client.kind() != BackendKind::Codex || !self.session.codex_scope().enabled() {
            return Err(failure(
                "older native task windows are available only for Codex",
            ));
        }
        match command {
            WindowCommand::Open {
                reader,
                expected,
                after,
            } => {
                let header = self
                    .session
                    .open_task_window(reader, &expected, after.as_ref())?;
                let scope = self.task_read_scope();
                panel::tick(
                    self.client.as_ref(),
                    &mut self.session,
                    scope,
                    &mut Default::default(),
                );
                Ok(EngineReply::TaskWindow(header))
            }
            WindowCommand::Read(context) => self
                .session
                .read_task_window(&context)
                .map(EngineReply::TaskWindow),
            WindowCommand::Close { reader, window } => {
                self.session.close_task_windows(&reader, window.as_ref());
                Ok(EngineReply::TaskWindowClosed)
            }
        }
    }

    pub(super) fn defer_task_window_read(&mut self, envelope: Envelope) -> Option<Envelope> {
        let EngineCommand::TaskWindow(WindowCommand::Read(context)) = &envelope.command else {
            return Some(envelope);
        };
        if !self
            .session
            .read_task_window(context)
            .is_ok_and(|header| header.reading)
        {
            return Some(envelope);
        }
        let duplicate = self.task_window_reads.iter().any(|pending| {
            matches!(&pending.command, EngineCommand::TaskWindow(WindowCommand::Read(previous))
                if previous.window == context.window)
        });
        if duplicate || self.task_window_reads.len() >= MAX_TASK_WINDOWS {
            let _ = envelope.reply.try_send(EngineReply::Failure(failure(
                "this native task window already has a pending read",
            )));
        } else {
            self.task_window_reads.push(envelope);
        }
        None
    }

    pub(super) fn finish_task_window_reads(&mut self) {
        self.session.expire_task_windows();
        let pending = std::mem::take(&mut self.task_window_reads);
        for envelope in pending {
            let EngineCommand::TaskWindow(WindowCommand::Read(context)) = &envelope.command else {
                unreachable!("only original window reads are deferred");
            };
            match self.session.read_task_window(context) {
                Ok(header) if header.reading => self.task_window_reads.push(envelope),
                result => {
                    let reply = result
                        .map(EngineReply::TaskWindow)
                        .unwrap_or_else(EngineReply::Failure);
                    let _ = envelope.reply.try_send(reply);
                    let _ = self.channels.notice.try_send(());
                }
            }
        }
    }
}
