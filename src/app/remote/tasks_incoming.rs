//! Redraw shared task models while retaining only this terminal's explicit stop-probe replies.

use carlos_client::panels::PanelKind;
use carlos_client::{Effects, ReadReply};

use super::Device;

impl Device {
    pub(super) fn apply_task_reads(&mut self, effects: &mut Effects) -> bool {
        let mut changed = effects.panels.iter().any(|change| {
            change.session == self.session
                && matches!(
                    change.kind,
                    PanelKind::Tasks
                        | PanelKind::TaskWindow
                        | PanelKind::TaskDetails
                        | PanelKind::TaskOverview
                        | PanelKind::TaskActivity
                )
        });
        if effects
            .panels
            .iter()
            .any(|change| change.session == self.session && change.kind == PanelKind::TaskWindow)
        {
            self.tasks.view.scroll = 0;
            self.tasks.view.error = None;
            self.clear_task_details();
        }
        for result in std::mem::take(&mut effects.replies) {
            match result.reply {
                ReadReply::TaskRun(run) => changed |= self.accept_task_run(result.id, run),
                _ => effects.replies.push(result),
            }
        }
        for failure in std::mem::take(&mut effects.failures) {
            let handled = failure
                .id
                .as_ref()
                .is_some_and(|id| self.fail_task_stop_read(id, &failure.error.message));
            changed |= handled;
            if !handled {
                effects.failures.push(failure);
            }
        }
        changed
    }
}
