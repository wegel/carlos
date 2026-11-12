//! Tracks native task runs and process-level membership without treating child edges as main turns.

use std::collections::{BTreeMap, HashMap, VecDeque};

use serde_json::{json, Value};

use super::task_stops::TaskStops;

const MAX_RECORDS: usize = 256;
const MAX_RUNS: usize = 4096;
const MAX_BYTES: usize = 8 * 1024 * 1024;

/// Keeps detailed recent task records and an independent authoritative live set.
#[derive(Default)]
#[cfg_attr(test, derive(Debug))]
pub(crate) struct ClaudeTasks {
    pub(in crate::session) records: BTreeMap<String, TaskRecord>,
    pub(in crate::session) live: BTreeMap<String, Value>,
    pub(in crate::session) stops: TaskStops,
    pub(in crate::session) control_error: Option<String>,
    runs: HashMap<String, String>,
    run_order: VecDeque<String>,
    sequence: u64,
    bytes: usize,
}

/// Retains the SDK's task metadata and the separate state of a stop request.
#[cfg_attr(test, derive(Debug))]
pub(crate) struct TaskRecord {
    pub(in crate::session) data: Value,
    pub(in crate::session) status: TaskStatus,
    sequence: u64,
    bytes: usize,
    terminal_notice: bool,
}

/// Describes actual native task state, independent of a control acknowledgement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TaskStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Stopped,
    Unknown,
}

impl ClaudeTasks {
    /// Replace the process live set, or merge a native task edge for its newest run.
    pub(in crate::session) fn update(
        &mut self,
        message: &Value,
    ) -> Result<Option<String>, &'static str> {
        if message["subtype"] == "background_tasks_changed" {
            self.replace_live(message)?;
            return Ok(None);
        }
        let Some(id) = message["task_id"].as_str() else {
            return Ok(None);
        };
        if id.len() > 4096
            || message["run_id"]
                .as_str()
                .is_some_and(|run| run.len() > 4096)
        {
            return Err("Native task identity exceeds the bounded task view");
        }
        if !self.accept_run(id, message["run_id"].as_str()) {
            return Ok(None);
        }
        if self.records.get(id).is_some_and(|record| {
            record.status.terminal()
                && record.data["run_id"] == message["run_id"]
                && (message["subtype"] != "task_notification" || record.terminal_notice)
        }) {
            return Ok(None);
        }
        let is_new = self
            .records
            .get(id)
            .is_none_or(|record| record.data["run_id"] != message["run_id"]);
        if is_new {
            self.insert(id, json!({"task_id":id,"run_id":message["run_id"]}));
        }
        self.merge_edge(id, message);
        self.trim();
        Ok(Some(id.to_string()))
    }

    /// Keep one stop request per native run even when the detail cache later evicts its row.
    pub(in crate::session) fn begin_stop(&mut self, id: &str) -> Option<Option<String>> {
        if !self.can_stop(id) {
            return None;
        }
        let run = self
            .data(id)?
            .get("run_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        match self.stops.begin(id, run.as_deref()) {
            Ok(true) => Some(run),
            Ok(false) => None,
            Err(error) => {
                self.control_error = Some(error.to_string());
                None
            }
        }
    }

    /// Apply an acknowledgement only while that exact task run remains nonterminal.
    pub(in crate::session) fn stop_reply(
        &mut self,
        id: &str,
        run: Option<&str>,
        error: Option<String>,
    ) {
        if self
            .data(id)
            .is_none_or(|data| data["run_id"].as_str() != run)
        {
            return;
        }
        if self
            .records
            .get(id)
            .is_some_and(|record| record.status.terminal())
        {
            return;
        }
        self.stops.reply(id, run, error);
    }

    fn merge_edge(&mut self, id: &str, message: &Value) {
        self.sequence += 1;
        if let Some(record) = self.records.get_mut(id) {
            self.bytes = self.bytes.saturating_sub(record.bytes);
            merge(&mut record.data, message);
            let patch = if message["subtype"] == "task_updated" {
                &message["patch"]
            } else {
                message
            };
            merge(&mut record.data, patch);
            if !record.status.terminal() {
                record.status = TaskStatus::parse(patch["status"].as_str()).unwrap_or_else(|| {
                    if message["subtype"] == "task_started" {
                        TaskStatus::Running
                    } else {
                        record.status
                    }
                });
            }
            if record.status.terminal() {
                self.stops.finish(id, record.data["run_id"].as_str());
            }
            record.terminal_notice |= message["subtype"] == "task_notification";
            record.sequence = self.sequence;
            record.bytes = record.data.to_string().len();
            self.bytes += record.bytes;
        }
    }

    fn replace_live(&mut self, message: &Value) -> Result<(), &'static str> {
        let tasks = message["tasks"]
            .as_array()
            .ok_or("Native task membership has no task list")?;
        if tasks.len() > MAX_RUNS {
            return Err("Native task membership exceeds 4096 tasks; stop the session to keep task controls bounded");
        }
        let mut next = BTreeMap::new();
        for task in tasks {
            let Some(id) = task["task_id"].as_str() else {
                continue;
            };
            if id.len() > 4096 || task["run_id"].as_str().is_some_and(|run| run.len() > 4096) {
                return Err("Native task identity exceeds the bounded task view");
            }
            if !self.accept_run(id, task["run_id"].as_str()) {
                if let Some(current) = self.live.get(id) {
                    next.insert(id.to_string(), current.clone());
                }
                continue;
            }
            next.insert(id.to_string(), task.clone());
            if self
                .records
                .get(id)
                .is_none_or(|record| record.data["run_id"] != task["run_id"])
            {
                self.insert(id, task.clone());
            }
            if let Some(record) = self.records.get_mut(id) {
                self.bytes = self.bytes.saturating_sub(record.bytes);
                merge(&mut record.data, task);
                record.bytes = record.data.to_string().len();
                self.bytes += record.bytes;
            }
        }
        self.live = next;
        self.trim();
        Ok(())
    }

    fn accept_run(&mut self, id: &str, run: Option<&str>) -> bool {
        let previous = self
            .runs
            .get(id)
            .map(String::as_str)
            .into_iter()
            .chain(self.live.get(id).and_then(|task| task["run_id"].as_str()))
            .chain(
                self.records
                    .get(id)
                    .and_then(|record| record.data["run_id"].as_str()),
            )
            .max_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
        if let Some(previous) = previous {
            let Some(run) = run else {
                return false;
            };
            if run.encode_utf16().cmp(previous.encode_utf16()).is_lt() {
                return false;
            }
        }
        if let Some(run) = run {
            if !self.runs.contains_key(id) {
                self.run_order.push_back(id.to_string());
            }
            self.runs.insert(id.to_string(), run.to_string());
            while self.run_order.len() > MAX_RUNS {
                if let Some(oldest) = self.run_order.pop_front() {
                    self.runs.remove(&oldest);
                }
            }
        }
        true
    }

    fn insert(&mut self, id: &str, data: Value) {
        if let Some(previous) = self.records.remove(id) {
            self.bytes = self.bytes.saturating_sub(previous.bytes);
        }
        self.sequence += 1;
        let bytes = data.to_string().len();
        self.bytes += bytes;
        self.records.insert(
            id.to_string(),
            TaskRecord {
                data,
                status: TaskStatus::Unknown,
                sequence: self.sequence,
                bytes,
                terminal_notice: false,
            },
        );
    }

    fn trim(&mut self) {
        while self.records.len() > MAX_RECORDS || self.bytes > MAX_BYTES {
            let oldest = self
                .records
                .iter()
                .min_by_key(|(_, record)| (!record.status.terminal(), record.sequence))
                .map(|(id, _)| id.clone());
            let Some(oldest) = oldest else {
                break;
            };
            if let Some(record) = self.records.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(record.bytes);
            }
        }
    }
}

impl TaskStatus {
    /// Identify native terminal edges so later progress cannot resurrect a run.
    pub(in crate::session) fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Stopped)
    }

    fn parse(status: Option<&str>) -> Option<Self> {
        Some(match status? {
            "pending" => Self::Pending,
            "running" => Self::Running,
            "paused" => Self::Paused,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "stopped" | "killed" => Self::Stopped,
            _ => return None,
        })
    }
}

fn merge(target: &mut Value, patch: &Value) {
    if let (Some(target), Some(patch)) = (target.as_object_mut(), patch.as_object()) {
        target.extend(
            patch
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
    }
}
