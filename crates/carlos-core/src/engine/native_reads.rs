//! Read bounded native task views without retaining any client's selected row or page cursor.

use std::sync::atomic::Ordering;

use carlos_proto::commands::RpcError;
use carlos_proto::ids::{TaskId, TaskReadId, TaskRevision};
use carlos_proto::task_facts::TaskSnapshot;

use super::owner::{failure, Owner};
use super::{EngineCommand, EngineReply};
use crate::backend::BackendKind;
use crate::session::codex::panel;
use crate::session::codex::panel_scope::PanelScope;

impl Owner {
    pub(super) fn read_native(&mut self, command: EngineCommand) -> Result<EngineReply, RpcError> {
        match command {
            EngineCommand::TaskWindow(command) => self.read_task_window(command),
            EngineCommand::SettingsRead {
                expected,
                offset,
                limit,
            } => {
                self.publish();
                self.session
                    .settings_page(expected.as_ref(), offset, limit)
                    .map(EngineReply::Settings)
            }
            EngineCommand::GoalRead { refresh } => self.read_goal(refresh),
            command @ (EngineCommand::QueueRead { .. } | EngineCommand::QueueText { .. }) => {
                self.read_queue(command)
            }
            command @ (EngineCommand::TaskSnapshot
            | EngineCommand::TaskOverview { .. }
            | EngineCommand::TaskOverviewText { .. }
            | EngineCommand::TaskRun { .. }
            | EngineCommand::TaskList { .. }
            | EngineCommand::TaskDetail { .. }
            | EngineCommand::TaskText { .. }) => self.read_tasks(command),
            _ => Err(failure("mutation entered the read lane")),
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
            EngineCommand::TaskSnapshot => self.read_task_snapshot(),
            command @ (EngineCommand::TaskOverview { .. }
            | EngineCommand::TaskOverviewText { .. }) => self.read_task_overview(command),
            EngineCommand::TaskRun {
                window,
                task,
                read,
                refresh,
            } => self.read_task_run_in(window.as_ref(), task, read, refresh),
            EngineCommand::TaskList {
                window,
                expected,
                after,
                limit,
                refresh,
            } => {
                if let Some(window) = window {
                    if refresh {
                        return Err(failure("refresh the latest task list explicitly"));
                    }
                    self.session
                        .task_page_in(Some(&window), expected.as_ref(), after.as_ref(), limit)
                        .map(EngineReply::TaskList)
                } else {
                    self.list_tasks(expected, after, limit, refresh)
                }
            }
            EngineCommand::TaskDetail {
                window,
                expected,
                task,
            } => self
                .session
                .task_detail_in(window.as_ref(), &expected, &task)
                .map(EngineReply::TaskDetail),
            EngineCommand::TaskText {
                window,
                expected,
                task,
                field,
                offset,
                limit,
            } => self
                .session
                .task_text_in(window.as_ref(), &expected, &task, field, offset, limit)
                .map(EngineReply::TaskText),
            _ => unreachable!("typed task read"),
        }
    }

    fn read_task_snapshot(&mut self) -> Result<EngineReply, RpcError> {
        if self.client.kind() == BackendKind::Codex {
            let scope = self.task_read_scope();
            if self.session.codex_panel.snapshot.is_none() {
                panel::refresh(&mut self.session, scope, false);
            }
            panel::tick(
                self.client.as_ref(),
                &mut self.session,
                scope,
                &mut Default::default(),
            );
        }
        self.publish();
        let tasks = self
            .session
            .export_tasks()
            .ok_or_else(|| failure("native task facts are unavailable"))?;
        Ok(EngineReply::TaskSnapshot(TaskSnapshot {
            session: self.session.session_id().clone(),
            seq: self.session.events.seq,
            tasks,
        }))
    }

    fn read_task_run_in(
        &mut self,
        window: Option<&crate::session::task_windows::WindowContext>,
        task: TaskId,
        read: Option<TaskReadId>,
        refresh: bool,
    ) -> Result<EngineReply, RpcError> {
        self.session
            .prepare_task_run_in(window, &task, read.as_ref(), refresh)?;
        let scope = self.task_read_scope();
        panel::tick(
            self.client.as_ref(),
            &mut self.session,
            scope,
            &mut Default::default(),
        );
        self.publish();
        self.session
            .task_run_snapshot_in(window, &task)
            .map(EngineReply::TaskRun)
    }

    fn read_task_overview(&mut self, command: EngineCommand) -> Result<EngineReply, RpcError> {
        match command {
            EngineCommand::TaskOverview {
                expected,
                after,
                limit,
            } => self
                .session
                .task_overview_page(expected.as_ref(), after.as_ref(), limit)
                .map(EngineReply::TaskOverview),
            EngineCommand::TaskOverviewText {
                expected,
                entry,
                field,
                offset,
                limit,
            } => self
                .session
                .task_overview_text(&expected, &entry, field, offset, limit)
                .map(EngineReply::TaskOverviewText),
            _ => unreachable!("typed task overview"),
        }
    }

    fn list_tasks(
        &mut self,
        expected: Option<TaskRevision>,
        after: Option<TaskId>,
        limit: usize,
        refresh: bool,
    ) -> Result<EngineReply, RpcError> {
        if refresh && (expected.is_some() || after.is_some()) {
            return Err(failure("refresh cannot continue an old task page"));
        }
        if self.client.kind() == BackendKind::Codex && (refresh || !self.session.task_known()) {
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

    /// Background reads keep their own original targets; no daemon-owned panel focus exists.
    pub(super) fn task_read_scope(&self) -> PanelScope<'static> {
        let mut scope = self.session.backend_context().panel;
        scope.visible = self.channels.task_interest.load(Ordering::SeqCst);
        scope
    }
}
