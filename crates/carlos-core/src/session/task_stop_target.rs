//! Capture exact native stop targets and compare process identities without scanning their bodies.

use carlos_proto::tasks::TaskKind;
use serde_json::{json, Number, Value};

use super::codex::panel_state::Process;
use super::state::SessionState;
use crate::backend::BackendKind;

/// Native parameters remain private; public requests supply only task and run addresses.
#[derive(Debug)]
pub(in crate::session) enum StopTarget {
    Agent {
        thread: String,
        turn: String,
    },
    Process {
        thread: String,
        process: String,
        item: Option<String>,
        pid: Option<Number>,
    },
    Claude {
        task: String,
        run: String,
    },
}

impl StopTarget {
    pub fn kind(&self) -> TaskKind {
        match self {
            Self::Agent { .. } => TaskKind::Agent,
            Self::Process { .. } => TaskKind::Process,
            Self::Claude { .. } => TaskKind::Task,
        }
    }
    pub fn backend(&self) -> BackendKind {
        match self {
            Self::Agent { .. } | Self::Process { .. } => BackendKind::Codex,
            Self::Claude { .. } => BackendKind::Claude,
        }
    }

    pub fn request(&self) -> (&'static str, Value) {
        match self {
            Self::Agent { thread, turn } => {
                ("turn/interrupt", json!({"threadId":thread,"turnId":turn}))
            }
            Self::Process {
                thread, process, ..
            } => (
                "thread/backgroundTerminals/terminate",
                json!({"threadId":thread,"processId":process}),
            ),
            Self::Claude { task, run } => ("task.stop", json!({"taskId":task,"runId":run})),
        }
    }

    pub fn process_is_current(&self, session: &SessionState) -> bool {
        let Self::Process { thread, .. } = self else {
            return true;
        };
        session.process_known(thread)
            && session
                .codex_panel
                .processes
                .iter()
                .any(|row| self.matches_process(row))
            && (thread == session.thread_id()
                || session.codex_panel.ownership.contains(
                    session.thread_id(),
                    session.codex_scope().epoch(),
                    thread,
                ))
    }

    pub fn matches_process(&self, row: &Process) -> bool {
        let Self::Process {
            thread,
            process,
            item,
            pid,
        } = self
        else {
            return false;
        };
        let Some((current_item, current_pid)) = process_identity(&row.data) else {
            return false;
        };
        row.thread == *thread
            && row.data["processId"] == *process
            && current_item == item.as_deref()
            && current_pid == pid.as_ref()
    }

    pub fn acknowledged(&self, session: &mut SessionState) {
        if let Self::Process {
            thread,
            process,
            item,
            ..
        } = self
        {
            if session
                .codex_panel
                .processes
                .iter()
                .any(|row| self.matches_process(row))
            {
                super::codex::task_status::terminated(session, thread, process, item.as_deref());
            }
        }
    }
}

/// Require a supported item/PID pair before promising a guarded process stop.
pub(in crate::session) fn process_identity(
    data: &Value,
) -> Option<(Option<&str>, Option<&Number>)> {
    let item = match data.get("itemId") {
        None | Some(Value::Null) => None,
        Some(Value::String(id)) if !id.is_empty() && id.len() <= 4096 => Some(id.as_str()),
        _ => return None,
    };
    let pid = match data.get("osPid") {
        None | Some(Value::Null) => None,
        Some(Value::Number(pid)) if pid.is_u64() => Some(pid),
        _ => return None,
    };
    (item.is_some() || pid.is_some()).then_some((item, pid))
}
