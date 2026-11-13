//! Borrow native task details and counts without granting callers write access.

use std::collections::BTreeMap;

use serde_json::Value;

use super::task_stops::TaskStops;
use super::tasks::{ClaudeTasks, TaskRecord, TaskStatus};

impl ClaudeTasks {
    /// Count only non-ambient members of the latest process-level set as activity.
    pub fn active_count(&self) -> usize {
        self.live
            .values()
            .filter(|task| {
                let record = task["task_id"].as_str().and_then(|id| self.records.get(id));
                let ambient = task["ambient"] == true
                    || task["skip_transcript"] == true
                    || record.is_some_and(|record| {
                        record.data["ambient"] == true || record.data["skip_transcript"] == true
                    });
                !ambient
                    && !record.is_some_and(|record| {
                        record.status.terminal() && record.data["run_id"] == task["run_id"]
                    })
            })
            .count()
    }

    /// Include every live task in the view, even if its detailed record was evicted.
    pub fn ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .records
            .keys()
            .chain(self.live.keys())
            .cloned()
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Resolve the newest stored metadata for a row, with level-only records as fallback.
    pub fn data(&self, id: &str) -> Option<&Value> {
        self.records
            .get(id)
            .map(|record| &record.data)
            .or_else(|| self.live.get(id))
    }

    /// Permit stops for known running foreground tasks and authoritative live background tasks.
    pub fn can_stop(&self, id: &str) -> bool {
        self.records.get(id).map_or_else(
            || self.live.contains_key(id),
            |record| {
                !record.status.terminal()
                    && (self.live.contains_key(id)
                        || matches!(
                            record.status,
                            TaskStatus::Pending | TaskStatus::Running | TaskStatus::Paused
                        ))
            },
        )
    }

    /// Borrow the retained native task details.
    pub fn records(&self) -> &BTreeMap<String, TaskRecord> {
        &self.records
    }

    /// Borrow the latest process membership.
    pub fn live(&self) -> &BTreeMap<String, Value> {
        &self.live
    }

    /// Borrow outstanding stop requests.
    pub fn stops(&self) -> &TaskStops {
        &self.stops
    }

    /// Borrow a backend error that prevented a stop request.
    pub fn control_error(&self) -> Option<&str> {
        self.control_error.as_deref()
    }
}

impl TaskRecord {
    /// Borrow the native task metadata.
    #[cfg(any(test, feature = "test-support"))]
    pub fn data(&self) -> &Value {
        &self.data
    }

    /// Read the native lifecycle independently of stop acknowledgements.
    pub fn status(&self) -> TaskStatus {
        self.status
    }
}
