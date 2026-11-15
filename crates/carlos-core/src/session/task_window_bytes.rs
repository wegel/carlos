//! Count encoded native metadata without allocating another body or rehashing transcript text.

use std::io::{self, Write};

use serde_json::Value;

use super::codex::panel_state::Snapshot;
use super::state::SessionState;

#[derive(Default)]
struct Count(usize);

impl Write for Count {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self.0.saturating_add(bytes.len());
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(in crate::session) fn encoded_bytes(value: &Value) -> usize {
    let mut count = Count::default();
    serde_json::to_writer(&mut count, value).expect("native task metadata");
    count.0
}

impl SessionState {
    pub(in crate::session) fn enforce_task_window_bytes(&mut self) {
        let latest = self.latest_task_bytes();
        while latest.saturating_add(self.task_windows.bytes())
            > carlos_proto::task_windows::MAX_NATIVE_TASK_BYTES
        {
            let Some(id) = self
                .task_windows
                .slots
                .iter()
                .filter(|(_, slot)| slot.bytes() > 0)
                .max_by_key(|(id, slot)| (slot.bytes(), id.as_str()))
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            let slot = self
                .task_windows
                .slots
                .get_mut(&id)
                .expect("retained native window");
            slot.contents = if let super::task_window_state::Contents::Reading(read) =
                &slot.contents
            {
                super::task_window_state::Contents::Retired {
                    request: read.request.clone(),
                    started: read.started,
                }
            } else {
                super::task_window_state::Contents::Failed("Latest native task metadata grew; this older window retired under the shared 8 MiB allowance. Previous display retained; r reads latest".into())
            };
        }
        self.prune_task_window_requests();
        self.task_stops.refresh_processes(self);
    }

    pub(in crate::session) fn latest_task_bytes(&mut self) -> usize {
        if let Some(bytes) = self.task_windows.latest_bytes {
            return bytes;
        }
        let bytes = self
            .codex_panel
            .agents
            .iter()
            .map(encoded_bytes)
            .sum::<usize>()
            + self
                .codex_panel
                .processes
                .iter()
                .map(|row| encoded_bytes(&row.data) + row.thread.len())
                .sum::<usize>()
            + scopes_bytes(&self.codex_panel.process_scopes)
            + self
                .codex_panel
                .reports
                .iter()
                .map(|(id, report)| id.len() + encoded_bytes(report))
                .sum::<usize>();
        self.task_windows.latest_bytes = Some(bytes);
        bytes
    }

    pub(in crate::session) fn task_window_bytes(&mut self) -> usize {
        self.latest_task_bytes()
            .saturating_add(self.task_windows.bytes())
    }

    pub(in crate::session) fn task_snapshot_bytes(&self, read: &Snapshot) -> usize {
        read.agents.iter().map(encoded_bytes).sum::<usize>()
            + read
                .processes
                .iter()
                .map(|row| encoded_bytes(&row.data) + row.thread.len())
                .sum::<usize>()
            + scopes_bytes(&read.process_scopes)
            + self
                .codex_panel
                .reports
                .iter()
                .map(|(id, report)| id.len() + encoded_bytes(report))
                .sum::<usize>()
    }
}

fn scopes_bytes(scopes: &std::collections::HashMap<String, Option<String>>) -> usize {
    scopes
        .iter()
        .map(|(owner, error)| owner.len() + error.as_ref().map_or(0, String::len))
        .sum()
}
