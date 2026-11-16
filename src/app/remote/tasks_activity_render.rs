//! Feed cached typed child records into the same activity painter used by embedded F2.

use std::borrow::Cow;

use super::Device;
use crate::app::child_activity_view::{self, ActivityRecord, ActivityView};

impl Device {
    pub(super) fn task_activity_lines(&self) -> Vec<String> {
        if !self.tasks.activity.show {
            return Vec::new();
        }
        let Some(activity) = self.client.task_activity(&self.session) else {
            return Vec::new();
        };
        let mut lines = Vec::new();
        if activity.is_stale() {
            lines.push(if self.tasks.windows.current.is_some() {
                "Child activity reader retired; accepted display kept. Esc returns to the older list; r refreshes latest tasks before Enter reopens".into()
            } else {
                "Child activity reader retired; accepted display kept. Use arrows to select a current task, then Enter to reopen.".into()
            });
        }
        if let Some(error) = activity.read_error() {
            lines.push(error.into());
        }
        let header = activity.header();
        let records = activity.records();
        lines.extend(child_activity_view::lines(
            ActivityView {
                error: header.and_then(|header| header.error.as_deref()),
                known: header.is_some_and(|header| header.known),
                reading: header.is_none_or(|header| header.reading) || activity.is_advancing(),
                count: header.map_or(records.len(), |header| usize::from(header.items)),
                older: header.is_some_and(|header| header.has_older),
                tail: header.map_or("", |header| header.tail.as_str()),
            },
            records.iter().map(|record| ActivityRecord {
                turn: &record.turn,
                item: &record.item,
                kind: &record.kind,
                text: Cow::Borrowed(&record.text),
                complete: record.complete,
            }),
        ));
        if activity.is_reading_pages() {
            lines.push("Reading accepted child display pages…".into());
        }
        lines
    }
}
