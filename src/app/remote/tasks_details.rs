//! Choose terminal field prefixes while the shared client owns requests and text assembly.

use anyhow::Result;
use carlos_client::panels::{TaskDetailTarget, TaskDetails, TaskFieldRead};
use carlos_proto::tasks::{TaskField, TaskKind};

use super::transport::Transport;
use super::Device;

pub(super) use carlos_client::panels::MAX_TASK_DETAIL_BYTES as MAX_DETAIL_BYTES;

impl Device {
    pub(super) fn task_details(&self) -> Option<&TaskDetails> {
        self.client.task_details(&self.session)
    }

    pub(super) fn task_detail_field(&self, field: TaskField) -> Option<&str> {
        displayed_field(self.task_details()?, field)
    }

    pub(super) fn clear_task_details(&mut self) {
        self.client.close_task_details(&self.session);
    }

    pub(super) fn drive_task_details(&mut self, link: &mut Transport) -> Result<()> {
        let Some(task) = self.tasks.pages.selected(self.tasks.view.selected.as_ref()) else {
            return Ok(());
        };
        let target = TaskDetailTarget {
            window: self.task_window_id(),
            expected: self
                .tasks
                .pages
                .revision()
                .cloned()
                .expect("accepted task page"),
            task: task.id.clone(),
        };
        if self
            .task_details()
            .is_none_or(|panel| panel.target() != &target)
        {
            let effects = self.client.open_task_details(&self.session, target)?;
            self.apply_effects(link, effects)?;
        }
        let Some(panel) = self
            .task_details()
            .filter(|panel| !panel.has_field_interest())
        else {
            return Ok(());
        };
        let Some(value) = panel.value() else {
            return Ok(());
        };
        let fields = value
            .properties
            .iter()
            .filter(|property| displayed(value.task.kind, property.field))
            .map(|property| TaskFieldRead {
                field: property.field,
                bytes: display_limit(value.task.kind, property.field),
            })
            .collect();
        let effects = self.client.read_task_fields(&self.session, fields)?;
        self.apply_effects(link, effects)?;
        Ok(())
    }
}

pub(super) fn displayed_field(details: &TaskDetails, field: TaskField) -> Option<&str> {
    let value = details.value()?;
    let property = value
        .properties
        .iter()
        .find(|property| property.field == field)?;
    let text = property.text.as_str();
    if property.complete || text.len() >= display_limit(value.task.kind, field).saturating_sub(3) {
        return Some(text);
    }
    let mut end = text.len().min(2048);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    Some(&text[..end])
}

fn displayed(kind: TaskKind, field: TaskField) -> bool {
    field != TaskField::Label && (kind != TaskKind::Agent || field != TaskField::Description)
}

fn display_limit(kind: TaskKind, field: TaskField) -> usize {
    if kind == TaskKind::Agent && field == TaskField::Report {
        65536
    } else {
        MAX_DETAIL_BYTES
    }
}
