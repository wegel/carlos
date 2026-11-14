//! Resolve opaque task and run addresses from the session's bounded native records.

use std::io::{self, Write};

use carlos_proto::ids::{TaskId, TaskRevision, TaskRun};
use carlos_proto::session::Backend;
use carlos_proto::tasks::{Task, TaskKind};
use serde_json::Value;

use super::codex::panel_state::Process;
use super::state::SessionState;
use crate::backend::BackendKind;

/// Borrow native metadata only for the duration of one owner read.
pub(super) enum TaskRef<'a> {
    Agent(&'a Value),
    Process(&'a Process),
    Claude { id: &'a str, data: &'a Value },
}

impl SessionState {
    pub(super) fn task_rows(&self) -> Vec<TaskRef<'_>> {
        match self.events.backend {
            Backend::Codex => self
                .codex_panel
                .agents
                .iter()
                .filter(|agent| agent["id"].as_str().is_some_and(|id| self.task_owns(id)))
                .map(TaskRef::Agent)
                .chain(
                    self.codex_panel
                        .processes
                        .iter()
                        .filter(|row| self.task_owns(&row.thread))
                        .map(TaskRef::Process),
                )
                .collect(),
            Backend::Claude => {
                let tasks = &self.claude.tasks;
                let mut rows: Vec<_> = tasks
                    .records()
                    .keys()
                    .map(String::as_str)
                    .chain(
                        tasks
                            .live()
                            .keys()
                            .map(String::as_str)
                            .filter(|id| !tasks.records().contains_key(*id)),
                    )
                    .filter_map(|id| tasks.data(id).map(|data| TaskRef::Claude { id, data }))
                    .collect();
                rows.sort_by(|left, right| left.native_id().cmp(right.native_id()));
                rows
            }
            Backend::Grok => Vec::new(),
        }
    }

    pub(super) fn task_revision(&self, rows: &[TaskRef<'_>]) -> TaskRevision {
        let mut hash = self.task_scope();
        hash.update(b"view");
        for row in rows {
            row.hash_identity(&mut hash);
            hash_value(&mut hash, row.data());
            if let TaskRef::Agent(agent) = row {
                if let Some(report) = agent["id"]
                    .as_str()
                    .and_then(|id| self.codex_panel.reports.get(id))
                {
                    hash_value(&mut hash, report);
                }
            }
        }
        hash.update(&[
            u8::from(self.task_known()),
            u8::from(self.codex_panel.snapshot.is_some()),
            u8::from(self.events.backend_closed),
        ]);
        hash_text(
            &mut hash,
            self.codex_panel.error.as_deref().unwrap_or_default(),
        );
        hash_value(&mut hash, &self.codex_panel.older);
        hash.finalize()
            .to_hex()
            .as_str()
            .parse()
            .expect("task revision digest")
    }

    pub(super) fn task_known(&self) -> bool {
        match self.events.backend {
            Backend::Codex => self.codex_panel.refreshed.is_some(),
            Backend::Claude => self.claude.enabled,
            Backend::Grok => false,
        }
    }

    fn task_owns(&self, id: &str) -> bool {
        id == self.thread_id()
            || self
                .codex_panel
                .ownership
                .contains(self.thread_id(), self.codex_scope().epoch(), id)
    }

    pub(super) fn task_scope(&self) -> blake3::Hasher {
        let (name, kind) = match self.events.backend {
            Backend::Codex => ("codex", BackendKind::Codex),
            Backend::Claude => ("claude", BackendKind::Claude),
            Backend::Grok => ("grok", BackendKind::Grok),
        };
        let mut hash = blake3::Hasher::new();
        hash.update(b"carlos.task.v1");
        for text in [self.session_id().as_str(), self.thread_id(), name] {
            hash_text(&mut hash, text);
        }
        let (root, helper) = self.command_connection(kind);
        hash.update(&root.to_le_bytes());
        hash.update(&helper.to_le_bytes());
        hash.update(&[u8::from(self.events.backend_closed)]);
        hash
    }
}

impl TaskRef<'_> {
    pub(super) fn id(&self, session: &SessionState) -> TaskId {
        let mut hash = session.task_scope();
        hash.update(b"task");
        self.hash_identity(&mut hash);
        hash.finalize()
            .to_hex()
            .as_str()
            .parse()
            .expect("task address digest")
    }

    pub(super) fn export(&self, session: &SessionState) -> Task {
        let (kind, state) = match self {
            Self::Agent(data) => (
                TaskKind::Agent,
                data["status"]["type"].as_str().unwrap_or("unknown"),
            ),
            Self::Process(_) => (TaskKind::Process, "present"),
            Self::Claude { id, .. } => (
                TaskKind::Task,
                session
                    .claude
                    .tasks
                    .records()
                    .get(*id)
                    .map(|record| match record.status() {
                        super::claude::TaskStatus::Pending => "pending",
                        super::claude::TaskStatus::Running => "running",
                        super::claude::TaskStatus::Paused => "paused",
                        super::claude::TaskStatus::Completed => "completed",
                        super::claude::TaskStatus::Failed => "failed",
                        super::claude::TaskStatus::Stopped => "stopped",
                        super::claude::TaskStatus::Unknown => "unknown",
                    })
                    .unwrap_or("unknown"),
            ),
        };
        let label = self.label();
        Task {
            id: self.id(session),
            kind,
            label: clipped(label, 1024).into(),
            label_complete: label.len() <= 1024,
            state: clipped(state, 128).into(),
            run: self.run(session),
        }
    }

    pub(super) fn label(&self) -> &str {
        match self {
            Self::Agent(data) => data["name"]
                .as_str()
                .or(data["preview"].as_str())
                .or(data["agentRole"].as_str())
                .unwrap_or("Agent"),
            Self::Process(row) => row.data["command"].as_str().unwrap_or("Background process"),
            Self::Claude { data, .. } => data["description"]
                .as_str()
                .or(data["summary"].as_str())
                .unwrap_or("Task"),
        }
    }

    pub(super) fn data(&self) -> &Value {
        match self {
            Self::Agent(data) | Self::Claude { data, .. } => data,
            Self::Process(row) => &row.data,
        }
    }

    fn native_id(&self) -> &str {
        match self {
            Self::Agent(data) => data["id"].as_str().unwrap_or_default(),
            Self::Process(row) => row.data["processId"].as_str().unwrap_or_default(),
            Self::Claude { id, .. } => id,
        }
    }

    fn hash_identity(&self, hash: &mut blake3::Hasher) {
        let tag = match self {
            Self::Agent(_) => "agent",
            Self::Process(_) => "process",
            Self::Claude { .. } => "task",
        };
        hash_text(hash, tag);
        if let Self::Process(row) = self {
            hash_text(hash, &row.thread);
        }
        hash_text(hash, self.native_id());
    }

    fn run(&self, session: &SessionState) -> Option<TaskRun> {
        let mut hash = session.task_scope();
        hash.update(b"run");
        self.hash_identity(&mut hash);
        match self {
            Self::Agent(data) => hash_text(
                &mut hash,
                data["lastObservedTurn"]["id"]
                    .as_str()
                    .filter(|id| !id.is_empty())?,
            ),
            Self::Claude { data, .. } => hash_text(
                &mut hash,
                data["run_id"].as_str().filter(|id| !id.is_empty())?,
            ),
            Self::Process(row) => {
                let item = &row.data["itemId"];
                let pid = &row.data["osPid"];
                if item.as_str().is_none_or(str::is_empty) && !pid.is_number() {
                    return None;
                }
                hash_value(&mut hash, item);
                hash_value(&mut hash, pid);
            }
        }
        Some(
            hash.finalize()
                .to_hex()
                .as_str()
                .parse()
                .expect("task run digest"),
        )
    }
}

pub(super) fn clipped(text: &str, limit: usize) -> &str {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn hash_text(hash: &mut blake3::Hasher, text: &str) {
    hash.update(&(text.len() as u64).to_le_bytes());
    hash.update(text.as_bytes());
}

fn hash_value(hash: &mut blake3::Hasher, value: &Value) {
    struct Digest<'a>(&'a mut blake3::Hasher);
    impl Write for Digest<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Digest(hash), value).expect("native JSON value");
    hash.update(b"\0");
}
