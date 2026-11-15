//! Freeze a selected task and complete its current-run read before one explicit native stop.

use anyhow::{ensure, Result};
use carlos_client::ConnectionId;
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::{TaskId, TaskReadId, TaskRevision};
use carlos_proto::tasks::{TaskKind, TaskRunSnapshot};

use super::tasks_stop_results::{OwnedStop, StopResults};
use super::transport::Transport;
use super::Device;

#[derive(Default)]
struct RunRead {
    request: Option<RequestId>,
    token: Option<TaskReadId>,
    seen: Option<TaskRevision>,
    reply: Option<TaskRunSnapshot>,
}

struct Intent {
    task: TaskId,
    connection: ConnectionId,
    read: RunRead,
}

/// Stop consent belongs to its original selected task and connection, never a replacement row.
#[derive(Default)]
pub(super) struct Stops {
    intent: Option<Intent>,
    pub(super) results: StopResults,
}

impl Stops {
    pub(super) fn disconnected(&mut self) -> bool {
        self.intent.take().is_some()
    }
    pub(super) fn waiting_for(&self, task: &TaskId) -> bool {
        self.intent
            .as_ref()
            .is_some_and(|intent| &intent.task == task)
    }
}

impl Device {
    pub(super) fn begin_task_stop(&mut self, link: Option<&mut Transport>) -> Result<()> {
        let link = link.ok_or_else(|| {
            anyhow::anyhow!("Disconnected; F5 reconnects before x rereads the native run")
        })?;
        ensure!(
            self.client.session_is_ready(link.connection, &self.session),
            "Wait for accepted session facts before stopping a task"
        );
        ensure!(
            self.tasks.stops.intent.is_none(),
            "A current-run read is already pending"
        );
        ensure!(
            self.task_facts()
                .is_some_and(|facts| facts.known && !facts.reading && !facts.closed)
                && self.tasks.pages.revision.as_ref()
                    == self.task_facts().map(|facts| &facts.revision),
            "Refresh current task facts before stopping"
        );
        let task = self
            .tasks
            .pages
            .selected(self.tasks.view.selected.as_ref())
            .ok_or_else(|| {
                anyhow::anyhow!("Selected row unavailable; use arrows before stopping")
            })?;
        ensure!(
            !self.tasks.stops.results.pending(&self.client, &task.id),
            "This task stop is already pending"
        );
        if task.kind != TaskKind::Agent {
            ensure!(
                self.tasks
                    .details
                    .field(carlos_proto::tasks::TaskField::CanStop)
                    == Some("true"),
                "Stop unavailable for this native task"
            );
        }
        self.request_stop_run(link, task.id.clone(), task.kind == TaskKind::Agent)
    }

    fn request_stop_run(
        &mut self,
        link: &mut Transport,
        task: TaskId,
        refresh: bool,
    ) -> Result<()> {
        let outgoing = self.client.read(
            link.connection,
            Command::TaskRun {
                window: None,
                session: self.session.clone(),
                task: task.clone(),
                read: None,
                refresh,
            },
        )?;
        self.tasks.stops.intent = Some(Intent {
            task: task.clone(),
            connection: link.connection,
            read: RunRead {
                request: Some(outgoing.id()),
                seen: self.task_facts().map(|facts| facts.revision.clone()),
                ..Default::default()
            },
        });
        self.tasks.view.error = None;
        self.tasks.paint_dirty.set(true);
        link.send(outgoing)
    }

    pub(super) fn accept_task_run(&mut self, id: Option<RequestId>, run: TaskRunSnapshot) -> bool {
        let Some(intent) = self.tasks.stops.intent.as_mut() else {
            return false;
        };
        if intent.read.request.as_ref() != id.as_ref() || id.is_none() {
            return false;
        }
        intent.read.request = None;
        intent.read.reply = Some(run);
        true
    }

    pub(super) fn fail_task_stop_read(&mut self, id: &RequestId, message: &str) -> bool {
        if !self
            .tasks
            .stops
            .intent
            .as_ref()
            .is_some_and(|intent| intent.read.request.as_ref() == Some(id))
        {
            return false;
        }
        self.tasks.stops.intent = None;
        self.tasks.view.error = Some(format!("Native run read failed; no stop sent: {message}"));
        true
    }

    pub(super) fn drive_task_stop(&mut self, link: &mut Transport) -> Result<()> {
        let Some(mut intent) = self.tasks.stops.intent.take() else {
            return Ok(());
        };
        if intent.connection != link.connection
            || !self.client.session_is_ready(link.connection, &self.session)
        {
            self.tasks.view.error =
                Some("Native run read interrupted; x explicitly rereads before stopping".into());
            return Ok(());
        }
        if let Some(run) = intent.read.reply.take() {
            ensure!(
                run.task == intent.task,
                "Native run reply changed its selected task"
            );
            if let Some(error) = run.error {
                self.tasks.view.error =
                    Some(format!("Native run read failed; no stop sent: {error}"));
                return Ok(());
            }
            if run.closed {
                self.tasks.view.error = Some("Native task connection closed; no stop sent".into());
                return Ok(());
            }
            if !run.reading {
                return self.submit_task_stop(link, intent.task, run);
            }
            if run.read.is_none() {
                self.tasks.view.error =
                    Some("Native run read has no original read identity; no stop sent".into());
                return Ok(());
            }
            intent.read.token = run.read;
        }
        let revision = self.task_facts().map(|facts| facts.revision.clone());
        if intent.read.request.is_none()
            && intent.read.token.is_some()
            && revision != intent.read.seen
        {
            self.continue_task_run(link, &mut intent, revision)?;
        }
        self.tasks.stops.intent = Some(intent);
        Ok(())
    }

    fn continue_task_run(
        &mut self,
        link: &mut Transport,
        intent: &mut Intent,
        revision: Option<TaskRevision>,
    ) -> Result<()> {
        let outgoing = self.client.read(
            link.connection,
            Command::TaskRun {
                window: None,
                session: self.session.clone(),
                task: intent.task.clone(),
                read: intent.read.token.clone(),
                refresh: false,
            },
        )?;
        intent.read.request = Some(outgoing.id());
        intent.read.seen = revision;
        link.send(outgoing)
    }

    fn submit_task_stop(
        &mut self,
        link: &mut Transport,
        task: TaskId,
        run: TaskRunSnapshot,
    ) -> Result<()> {
        let Some(expected_run) = run.run.filter(|_| run.known) else {
            self.tasks.view.error = Some("No current native run was returned; no stop sent".into());
            return Ok(());
        };
        let session = self.session.clone();
        let mut identity = None;
        let outgoing = self.client.submit(link.connection, |operation| {
            identity = Some(operation.clone());
            Command::TaskStop {
                window: None,
                session,
                operation,
                task: task.clone(),
                expected_run: expected_run.clone(),
            }
        })?;
        let operation = identity.expect("allocated private task stop");
        self.tasks.stops.results.owned.push((
            operation,
            OwnedStop {
                task,
                run: expected_run,
            },
        ));
        let id = outgoing.id();
        if let Err(error) = link.send(outgoing) {
            let effects = self
                .client
                .write_failed(link.connection, id, &error.to_string(), false);
            self.apply_operations(&effects.operations);
            return Err(error);
        }
        Ok(())
    }
}
