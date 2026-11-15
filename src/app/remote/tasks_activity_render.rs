//! Feed cached typed child records into the same activity painter used by embedded F2.

use std::borrow::Cow;

use super::Device;
use crate::app::child_activity_view::{self, ActivityRecord, ActivityView};

impl Device {
    pub(super) fn task_activity_lines(&self) -> Vec<String> {
        let activity = &self.tasks.activity;
        if !activity.show {
            return Vec::new();
        }
        let mut lines = Vec::new();
        if activity.stale {
            lines.push(if self.tasks.windows.current.is_some() {
                "Child activity reader retired; accepted display kept. Esc returns to the older list; r refreshes latest tasks before Enter reopens".into()
            } else {
                "Child activity reader retired; accepted display kept. Enter explicitly reopens its selected task after the current task page is accepted".into()
            });
        }
        if let Some(error) = &activity.error {
            lines.push(error.clone());
        }
        let header = activity.header.as_ref();
        let records = activity
            .accepted
            .as_ref()
            .map(|display| display.records.as_slice())
            .unwrap_or_default();
        lines.extend(child_activity_view::lines(
            ActivityView {
                error: header.and_then(|header| header.error.as_deref()),
                known: header.is_some_and(|header| header.known),
                reading: header.is_none_or(|header| header.reading)
                    || activity.requests.advance.is_some(),
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
        if activity.pending.is_some() {
            lines.push("Reading accepted child display pages…".into());
        }
        lines
    }
}
