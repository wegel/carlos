//! Read only named native task fields and preserve full values for bounded exact text reads.

use std::borrow::Cow;

use carlos_proto::tasks::TaskField;
use serde_json::Value;

use super::state::SessionState;
use super::task_rows::TaskRef;

pub(super) const FIELDS: [TaskField; 31] = [
    TaskField::Label,
    TaskField::NativeId,
    TaskField::Owner,
    TaskField::LastTurn,
    TaskField::ProcessItem,
    TaskField::RunLabel,
    TaskField::CanStop,
    TaskField::NativeState,
    TaskField::Role,
    TaskField::Parent,
    TaskField::DirectInput,
    TaskField::LastTurnState,
    TaskField::Report,
    TaskField::ReportState,
    TaskField::Command,
    TaskField::Directory,
    TaskField::OsPid,
    TaskField::CpuPercent,
    TaskField::RssKib,
    TaskField::Description,
    TaskField::Summary,
    TaskField::NativeType,
    TaskField::ParentCaller,
    TaskField::AgentType,
    TaskField::OutputFile,
    TaskField::Error,
    TaskField::UsageTokens,
    TaskField::UsageTools,
    TaskField::UsageDurationMillis,
    TaskField::Ambient,
    TaskField::SkipTranscript,
];

impl TaskRef<'_> {
    pub(super) fn field<'a>(
        &'a self,
        session: &'a SessionState,
        field: TaskField,
    ) -> Option<Cow<'a, str>> {
        match field {
            TaskField::Label => return Some(Cow::Borrowed(self.label())),
            TaskField::NativeId => return Some(Cow::Borrowed(self.native_id())),
            TaskField::CanStop => return Some(Cow::Owned(self.can_stop(session).to_string())),
            TaskField::NativeState => return Some(self.state_text(session)),
            TaskField::LastTurn => return self.agent_run().map(|(id, _)| Cow::Borrowed(id)),
            TaskField::LastTurnState => {
                return self.agent_run().map(|(_, state)| Cow::Borrowed(state))
            }
            TaskField::Owner => {
                return if let Self::Process(row) | Self::WindowProcess { row, .. } = self {
                    Some(Cow::Borrowed(&row.thread))
                } else {
                    None
                }
            }
            _ => {}
        }
        if field == TaskField::Error {
            if let Self::WindowProcess { error, .. } = self {
                return error.map(Cow::Borrowed);
            }
            if let Self::Process(row) = self {
                return session.process_read_error(&row.thread).map(Cow::Borrowed);
            }
        }
        let value = match self {
            Self::Agent { data, report, .. } => agent_field(data, *report, field)?,
            Self::Process(row) | Self::WindowProcess { row, .. } => {
                &row.data[match field {
                    TaskField::ProcessItem => "itemId",
                    TaskField::Command => "command",
                    TaskField::Directory => "cwd",
                    TaskField::OsPid => "osPid",
                    TaskField::CpuPercent => "cpuPercent",
                    TaskField::RssKib => "rssKb",
                    _ => return None,
                }]
            }
            Self::Claude { data, .. } => claude_field(data, field)?,
        };
        scalar(value)
    }

    fn can_stop(&self, session: &SessionState) -> bool {
        if session.events.backend_closed {
            return false;
        }
        match self {
            Self::Agent { .. } => true,
            Self::Process(_) | Self::WindowProcess { .. } => self.run(session).is_some(),
            Self::Claude { id, .. } => {
                session.claude.tasks.can_stop(id) && self.run(session).is_some()
            }
        }
    }

    fn state_text<'a>(&'a self, session: &'a SessionState) -> Cow<'a, str> {
        let state = self.native_state(session);
        let Self::Agent { data, .. } = self else {
            return Cow::Borrowed(state);
        };
        let flags: Vec<_> = data["status"]["activeFlags"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        if flags.is_empty() {
            Cow::Borrowed(state)
        } else {
            Cow::Owned(format!("{state} ({})", flags.join(", ")))
        }
    }
}

fn agent_field<'a>(
    data: &'a Value,
    report: Option<&'a Value>,
    field: TaskField,
) -> Option<&'a Value> {
    Some(match field {
        TaskField::Role => &data["agentRole"],
        TaskField::Parent => &data["parentThreadId"],
        TaskField::DirectInput => &data["canAcceptDirectInput"],
        TaskField::Report | TaskField::ReportState => {
            let report = report?;
            &report[if field == TaskField::Report {
                "message"
            } else {
                "status"
            }]
        }
        _ => return None,
    })
}

fn claude_field(data: &Value, field: TaskField) -> Option<&Value> {
    let (key, nested) = match field {
        TaskField::RunLabel => ("run_id", None),
        TaskField::Description => ("description", None),
        TaskField::Summary => ("summary", None),
        TaskField::NativeType => ("task_type", None),
        TaskField::Parent => ("parent_task_id", None),
        TaskField::ParentCaller => ("parent_tool_use_id", None),
        TaskField::AgentType => ("subagent_type", None),
        TaskField::OutputFile => ("output_file", None),
        TaskField::Error => ("error", None),
        TaskField::Ambient => ("ambient", None),
        TaskField::SkipTranscript => ("skip_transcript", None),
        TaskField::UsageTokens => ("usage", Some("total_tokens")),
        TaskField::UsageTools => ("usage", Some("tool_uses")),
        TaskField::UsageDurationMillis => ("usage", Some("duration_ms")),
        _ => return None,
    };
    Some(nested.map_or(&data[key], |nested| &data[key][nested]))
}

fn scalar(value: &Value) -> Option<Cow<'_, str>> {
    match value {
        Value::String(text) => Some(Cow::Borrowed(text)),
        Value::Bool(_) | Value::Number(_) => Some(Cow::Owned(value.to_string())),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}
