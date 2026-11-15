//! Keep the embedded picker bridge until the separate embedded-path deletion commit.

use carlos_core::session::models::ThreadSummary;

use super::{draw_picker_rows, PickerRow};

impl<'a> From<&'a ThreadSummary> for PickerRow<'a> {
    fn from(row: &'a ThreadSummary) -> Self {
        Self {
            id: &row.id,
            name: row.name.as_deref(),
            preview: &row.preview,
            cwd: &row.cwd,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

pub(in crate::app) fn draw_picker(
    frame: &mut ratatui::Frame<'_>,
    threads: &[ThreadSummary],
    selected: usize,
    top: usize,
    allow_delete: bool,
    delete_target: Option<&ThreadSummary>,
    status: Option<&str>,
) {
    let rows: Vec<_> = threads.iter().map(PickerRow::from).collect();
    let target = delete_target.map(PickerRow::from);
    draw_picker_rows(
        frame,
        &rows,
        selected,
        top,
        allow_delete,
        target.as_ref(),
        status,
    );
}
