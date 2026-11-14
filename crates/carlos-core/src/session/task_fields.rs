//! Read only named native task fields and preserve full values for bounded exact text reads.

use std::borrow::Cow;

use carlos_proto::tasks::TaskField;
use serde_json::Value;

use super::state::SessionState;
use super::task_rows::TaskRef;

pub(super) const FIELDS: [TaskField; 24] = [
    TaskField::Label,
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
        if field == TaskField::Label {
            return Some(Cow::Borrowed(self.label()));
        }
        let value = match self {
            Self::Agent(data) => agent_field(data, session, field)?,
            Self::Process(row) => {
                &row.data[match field {
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
}

fn agent_field<'a>(
    data: &'a Value,
    session: &'a SessionState,
    field: TaskField,
) -> Option<&'a Value> {
    Some(match field {
        TaskField::Role => &data["agentRole"],
        TaskField::Parent => &data["parentThreadId"],
        TaskField::DirectInput => &data["canAcceptDirectInput"],
        TaskField::LastTurnState => &data["lastObservedTurn"]["status"],
        TaskField::Report | TaskField::ReportState => {
            let report = session.codex_panel.reports.get(data["id"].as_str()?)?;
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
