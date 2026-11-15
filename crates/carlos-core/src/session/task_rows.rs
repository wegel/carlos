//! Resolve opaque task and run addresses from the session's bounded native records.

use carlos_proto::ids::{TaskId, TaskRevision, TaskRun};
use carlos_proto::session::Backend;
use carlos_proto::tasks::{Task, TaskKind};
use serde_json::Value;

use super::codex::panel_runs::{self, RunRead};
use super::codex::panel_state::Process;
use super::state::SessionState;
use super::task_digest::{hash_text, hash_value};
use crate::backend::BackendKind;

/// Borrow native metadata only for the duration of one owner read.
pub(super) enum TaskRef<'a> {
    Agent {
        data: &'a Value,
        accepted_run: Option<&'a RunRead>,
        report: Option<&'a Value>,
    },
    Process(&'a Process),
    WindowProcess {
        row: &'a Process,
        error: Option<&'a str>,
    },
    Claude {
        id: &'a str,
        data: &'a Value,
    },
}

impl SessionState {
    pub(super) fn task_rows(&self) -> Vec<TaskRef<'_>> {
        match self.events.backend {
            Backend::Codex => self
                .codex_panel
                .agents
                .iter()
                .filter(|agent| agent["id"].as_str().is_some_and(|id| self.task_owns(id)))
                .map(|data| TaskRef::Agent {
                    data,
                    report: data["id"]
                        .as_str()
                        .and_then(|id| self.codex_panel.reports.get(id)),
                    accepted_run: panel_runs::ready(
                        self,
                        data["id"].as_str().expect("verified agent id"),
                    ),
                })
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
        hash.update(self.task_run_stamp().as_bytes());
        for row in rows {
            row.hash_identity(&mut hash);
            hash_value(&mut hash, row.data());
            if let TaskRef::Process(process) = row {
                hash.update(&[u8::from(self.process_known(&process.thread))]);
                hash_text(
                    &mut hash,
                    self.process_read_error(&process.thread).unwrap_or_default(),
                );
            }
            if let Some((_, state)) = row.agent_run() {
                hash_text(&mut hash, state);
            }
            if let Some(run) = row.run(self) {
                hash_text(&mut hash, run.as_str());
            } else {
                hash_text(&mut hash, "");
            }

            if let TaskRef::Agent { data: agent, .. } = row {
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

    pub(super) fn process_known(&self, owner: &str) -> bool {
        !self.codex_scope().closed()
            && !self.codex_task_status.uncertain.contains(owner)
            && !self.codex_task_status.errors.contains_key(owner)
            && !self
                .codex_panel
                .process_scopes
                .get(owner)
                .is_some_and(Option::is_some)
    }

    pub(super) fn process_read_error(&self, owner: &str) -> Option<&str> {
        if self.process_known(owner) {
            return None;
        }
        self.codex_task_status
            .errors
            .get(owner)
            .map(String::as_str)
            .or_else(|| {
                self.codex_panel
                    .process_scopes
                    .get(owner)
                    .and_then(|error| error.as_deref())
            })
            .or(Some(
                "Carlos needs a fresh process list before stopping this process",
            ))
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
        let kind = match self {
            Self::Agent { .. } => TaskKind::Agent,
            Self::Process(_) | Self::WindowProcess { .. } => TaskKind::Process,
            Self::Claude { .. } => TaskKind::Task,
        };
        let state = self.native_state(session);
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

    pub(super) fn native_state<'a>(&'a self, session: &'a SessionState) -> &'a str {
        match self {
            Self::Agent { data, .. } => data["status"]["type"]
                .as_str()
                .or(data["status"].as_str())
                .unwrap_or("unknown"),
            Self::Process(row) => {
                if session.process_known(&row.thread) {
                    "present"
                } else {
                    "unknown"
                }
            }
            Self::WindowProcess { error, .. } => {
                if error.is_none() {
                    "present"
                } else {
                    "unknown"
                }
            }
            Self::Claude { id, .. } => session
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
        }
    }

    pub(super) fn label(&self) -> &str {
        match self {
            Self::Agent { data, .. } => data["agentNickname"]
                .as_str()
                .or(data["name"].as_str())
                .unwrap_or_else(|| self.native_id()),
            Self::Process(row) | Self::WindowProcess { row, .. } => {
                row.data["command"].as_str().unwrap_or("Background process")
            }
            Self::Claude { data, .. } => data["description"]
                .as_str()
                .or(data["summary"].as_str())
                .unwrap_or("Task"),
        }
    }

    pub(super) fn data(&self) -> &Value {
        match self {
            Self::Agent { data, .. } | Self::Claude { data, .. } => data,
            Self::Process(row) | Self::WindowProcess { row, .. } => &row.data,
        }
    }

    pub(super) fn native_id(&self) -> &str {
        match self {
            Self::Agent { data, .. } => data["id"].as_str().unwrap_or_default(),
            Self::Process(row) | Self::WindowProcess { row, .. } => {
                row.data["processId"].as_str().unwrap_or_default()
            }
            Self::Claude { id, .. } => id,
        }
    }

    fn hash_identity(&self, hash: &mut blake3::Hasher) {
        let tag = match self {
            Self::Agent { .. } => "agent",
            Self::Process(_) | Self::WindowProcess { .. } => "process",
            Self::Claude { .. } => "task",
        };
        hash_text(hash, tag);
        if let Self::Process(row) | Self::WindowProcess { row, .. } = self {
            hash_text(hash, &row.thread);
        }
        hash_text(hash, self.native_id());
    }

    pub(super) fn agent_run(&self) -> Option<(&str, &str)> {
        let Self::Agent {
            data, accepted_run, ..
        } = self
        else {
            return None;
        };
        if let Some(read) = accepted_run {
            return read
                .turn
                .as_ref()
                .map(|turn| (turn.id.as_str(), turn.status.as_str()));
        }
        panel_runs::native_turn(data)
    }

    pub(super) fn run(&self, session: &SessionState) -> Option<TaskRun> {
        let mut hash = session.task_scope();
        hash.update(b"run");
        self.hash_identity(&mut hash);
        match self {
            Self::Agent { .. } => hash_text(&mut hash, self.agent_run()?.0),
            Self::Claude { data, .. } => hash_text(
                &mut hash,
                data["run_id"].as_str().filter(|id| !id.is_empty())?,
            ),
            Self::Process(row) | Self::WindowProcess { row, .. } => {
                if matches!(self, Self::WindowProcess { error: Some(_), .. })
                    || matches!(self, Self::Process(_)) && !session.process_known(&row.thread)
                {
                    return None;
                }
                super::task_stop_target::process_identity(&row.data)?;
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
