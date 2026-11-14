//! Read bounded native task views without retaining any client's selected row or page cursor.

use carlos_proto::commands::RpcError;

use super::owner::{failure, Owner};
use super::{EngineCommand, EngineReply};
use crate::backend::BackendKind;
use crate::session::codex::panel;
use crate::session::codex::panel_scope::PanelScope;

impl Owner {
    pub(super) fn read_native(&mut self, command: EngineCommand) -> Result<EngineReply, RpcError> {
        match command {
            EngineCommand::GoalRead { refresh } => self.read_goal(refresh),
            command @ (EngineCommand::QueueRead { .. } | EngineCommand::QueueText { .. }) => {
                self.read_queue(command)
            }
            command => self.read_tasks(command),
        }
    }

    pub(super) fn read_tasks(&mut self, command: EngineCommand) -> Result<EngineReply, RpcError> {
        if !matches!(self.client.kind(), BackendKind::Codex | BackendKind::Claude) {
            return Err(failure("this backend does not support native task views"));
        }
        if self.client.kind() == BackendKind::Codex && !self.session.codex_scope().enabled() {
            return Err(failure("native Codex task controls are unavailable"));
        }
        match command {
            EngineCommand::TaskList {
                expected,
                after,
                limit,
                refresh,
            } => {
                if refresh && (expected.is_some() || after.is_some()) {
                    return Err(failure("refresh cannot continue an old task page"));
                }
                if self.client.kind() == BackendKind::Codex
                    && (refresh || !self.session.task_known())
                {
                    let scope = self.task_read_scope();
                    panel::refresh(&mut self.session, scope, false);
                    panel::tick(
                        self.client.as_ref(),
                        &mut self.session,
                        scope,
                        &mut Default::default(),
                    );
                    self.publish();
                }
                self.session
                    .task_page(expected.as_ref(), after.as_ref(), limit)
                    .map(EngineReply::TaskList)
            }
            EngineCommand::TaskDetail { expected, task } => self
                .session
                .task_detail(&expected, &task)
                .map(EngineReply::TaskDetail),
            EngineCommand::TaskText {
                expected,
                task,
                field,
                offset,
                limit,
            } => self
                .session
                .task_text(&expected, &task, field, offset, limit)
                .map(EngineReply::TaskText),
            _ => unreachable!("typed task read"),
        }
    }

    /// Background reads keep their own original targets; no daemon-owned panel focus exists.
    pub(super) fn task_read_scope(&self) -> PanelScope<'static> {
        let mut scope = self.session.backend_context().panel;
        scope.visible = false;
        scope
    }
}
