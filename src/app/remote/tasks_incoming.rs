//! Match task replies and read failures to this device's original private request identities.

use anyhow::Result;
use carlos_client::panels::PanelKind;
use carlos_client::{ClientError, Effects, ReadReply};
use carlos_proto::commands::RequestId;
use carlos_proto::task_facts::TaskSnapshot;

use super::Device;

impl Device {
    pub(super) fn apply_task_reads(&mut self, effects: &mut Effects) -> Result<bool> {
        let mut changed = effects.panels.iter().any(|change| {
            change.session == self.session
                && matches!(
                    change.kind,
                    PanelKind::TaskDetails | PanelKind::TaskOverview
                )
        });
        for result in std::mem::take(&mut effects.replies) {
            if result
                .id
                .as_ref()
                .is_some_and(|id| self.tasks.reads.take_retired(id))
            {
                continue;
            }
            match result.reply {
                ReadReply::TaskActivity(header) => {
                    changed |= self.accept_activity_header(result.id.as_ref(), header)
                }
                ReadReply::TaskActivityPage(page) => {
                    changed |= self.accept_activity_page(result.id.as_ref(), page)?
                }
                ReadReply::TaskWindow(header) => {
                    changed |= self.accept_task_window(result.id, header)
                }
                ReadReply::TaskSnapshot(snapshot) => {
                    changed |= self.accept_task_header(result.id, snapshot, effects)?;
                }
                ReadReply::TaskPage(page) => {
                    if self.accept_task_window_page(result.id.as_ref(), &page)? {
                        changed = true;
                        continue;
                    }
                    if self
                        .tasks
                        .reads
                        .request
                        .as_ref()
                        .is_some_and(|read| Some(&read.id) == result.id.as_ref())
                    {
                        let read = self.tasks.reads.request.take().expect("matching task page");
                        self.tasks.reads.future = Some((read, page));
                        changed = true;
                    }
                }
                ReadReply::TaskRun(run) => changed |= self.accept_task_run(result.id, run),
                _ => effects.replies.push(result),
            }
        }
        changed |= self.apply_task_read_failures(effects);
        Ok(changed)
    }
    fn accept_task_header(
        &mut self,
        id: Option<RequestId>,
        snapshot: TaskSnapshot,
        effects: &mut Effects,
    ) -> Result<bool> {
        let owned = id == self.tasks.reads.header;
        if owned {
            self.tasks.reads.header = None;
            self.tasks.reads.header_error = None;
        }
        match self.client.merge_tasks(&snapshot) {
            Ok(merged) => {
                effects.sessions.extend(merged.sessions);
                effects.outgoing.extend(merged.outgoing);
                effects.failures.extend(merged.failures);
                Ok(true)
            }
            Err(ClientError::StaleTasks) => {
                if owned
                    && self
                        .client
                        .session(&self.session)
                        .is_some_and(|replica| replica.sequence() < snapshot.seq)
                {
                    self.tasks.reads.waiting = Some(snapshot.seq);
                }
                Ok(false)
            }
            Err(error) => Err(error.into()),
        }
    }

    fn apply_task_read_failures(&mut self, effects: &mut Effects) -> bool {
        let mut changed = false;
        let sequence = self.task_sequence();
        for failure in std::mem::take(&mut effects.failures) {
            let handled = failure.id.as_ref().is_some_and(|id| {
                self.tasks.reads.take_retired(id)
                    || self.fail_task_activity(id, &failure.error.message)
                    || self.fail_task_window(id, &failure.error.message)
                    || self.tasks.reads.fail(id, &failure.error.message, sequence)
                    || self.fail_task_stop_read(id, &failure.error.message)
            });
            changed |= handled;
            if !handled {
                effects.failures.push(failure);
            }
        }
        changed
    }
}
