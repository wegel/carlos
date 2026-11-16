//! Keep one terminal's captured child activity, accepted display and pending reads private.

use anyhow::{ensure, Result};
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::{TaskId, TaskRevision};
use carlos_proto::task_activity::{TaskActivityHeader, TaskActivityRecord};
use carlos_proto::tasks::TaskKind;

use super::transport::Transport;
use super::Device;

pub(super) struct Display {
    pub(super) revision: TaskRevision,
    pub(super) records: Vec<TaskActivityRecord>,
    pub(super) bytes: usize,
    pub(super) more: bool,
}

#[derive(Default)]
pub(super) struct ActivityRequests {
    pub(super) header: Option<RequestId>,
    pub(super) page: Option<RequestId>,
    pub(super) advance: Option<RequestId>,
}

#[derive(Default)]
pub(super) struct ChildActivity {
    pub(super) show: bool,
    pub(super) task: Option<TaskId>,
    pub(super) header: Option<TaskActivityHeader>,
    pub(super) accepted: Option<Display>,
    pub(super) pending: Option<Display>,
    pub(super) requests: ActivityRequests,
    pub(super) stale: bool,
    pub(super) error: Option<String>,
    pub(super) failed_revision: Option<TaskRevision>,
    pub(super) close: bool,
    pub(super) outgoing: Vec<carlos_client::Outbound>,
}

impl ChildActivity {
    pub(super) fn retire(&mut self) -> Vec<RequestId> {
        self.pending = None;
        [
            self.requests.header.take(),
            self.requests.page.take(),
            self.requests.advance.take(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl Device {
    pub(super) fn open_task_activity(&mut self, link: Option<&mut Transport>) -> Result<()> {
        let link =
            link.ok_or_else(|| anyhow::anyhow!("Reconnect before reading child activity"))?;
        ensure!(
            self.client.session_is_ready(link.connection, &self.session),
            "Wait for accepted session facts before reading a child"
        );
        let selected = self
            .tasks
            .pages
            .selected(self.tasks.view.selected.as_ref())
            .ok_or_else(|| anyhow::anyhow!("Select an available child first"))?;
        if selected.kind != TaskKind::Agent {
            return Ok(());
        }
        if self.tasks.activity.show
            && !self.tasks.activity.stale
            && self.tasks.activity.header.is_some()
        {
            return self.advance_task_activity(link, false);
        }
        ensure!(
            self.task_view_is_current(),
            "Wait for the selected task's accepted page before opening activity"
        );
        let task = selected.id.clone();
        let expected = self
            .tasks
            .pages
            .revision()
            .cloned()
            .expect("accepted task revision");
        self.flush_activity_outgoing(link)?;
        self.prepare_activity_display(&task);
        let outgoing = self.client.read(
            link.connection,
            Command::TaskActivityOpen {
                session: self.session.clone(),
                task: task.clone(),
                expected,
                window: self.task_window_id(),
            },
        )?;
        self.tasks.activity.task = Some(task);
        self.tasks.activity.requests.header = Some(outgoing.id());
        link.send(outgoing)
    }

    fn prepare_activity_display(&mut self, task: &TaskId) {
        if self.tasks.activity.task.as_ref() != Some(task) {
            self.tasks.activity.accepted = None;
        }
        let retired = self.tasks.activity.retire();
        self.tasks.reads.retire(retired);
        self.tasks.activity.header = None;
        self.tasks.activity.show = true;
        self.tasks.activity.stale = false;
        self.tasks.activity.error = None;
        self.tasks.activity.failed_revision = None;
        self.tasks.view.scroll = 0;
        self.tasks.paint_dirty.set(true);
        self.tasks
            .activity
            .outgoing
            .extend(self.client.close_tasks(&self.session).outgoing);
    }

    pub(super) fn advance_task_activity(
        &mut self,
        link: &mut Transport,
        older: bool,
    ) -> Result<()> {
        ensure!(
            !self.tasks.activity.stale,
            "This child reader retired; Enter explicitly reopens its selected task"
        );
        ensure!(
            self.tasks.activity.requests.advance.is_none(),
            "The child read is already pending"
        );
        let header = self
            .tasks
            .activity
            .header
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("The original child reader is still opening"))?;
        ensure!(!header.reading, "The native child read is still pending");
        let outgoing = self.client.read(
            link.connection,
            Command::TaskActivityAdvance {
                session: self.session.clone(),
                activity: header.activity.clone(),
                older,
            },
        )?;
        self.tasks.activity.requests.advance = Some(outgoing.id());
        self.tasks.activity.failed_revision = None;
        self.tasks.activity.error = None;
        self.tasks.view.scroll = 0;
        self.tasks.paint_dirty.set(true);
        link.send(outgoing)
    }

    pub(super) fn close_task_activity(&mut self, restore_list: bool) -> Result<()> {
        self.tasks.activity.close |= self.tasks.activity.show && !self.tasks.activity.stale;
        self.tasks.activity.show = false;
        let retired = self.tasks.activity.retire();
        self.tasks.reads.retire(retired);
        self.tasks.activity.stale = true;
        self.tasks.view.scroll = 0;
        self.tasks.paint_dirty.set(true);
        if restore_list && self.tasks.windows.current.is_none() {
            self.tasks
                .activity
                .outgoing
                .extend(self.client.watch_tasks(&self.session)?.outgoing);
            self.tasks.reads.header = self.client.pending_task_read(&self.session);
        }
        Ok(())
    }

    pub(super) fn task_activity_disconnected(&mut self) {
        let retired = self.tasks.activity.retire();
        self.tasks.reads.retire(retired);
        self.tasks.activity.close = false;
        self.tasks.activity.outgoing.clear();
        self.tasks.activity.stale = true;
    }
}
