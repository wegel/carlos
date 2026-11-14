//! Format typed task fields and readonly overview rows with the existing native panel wording.

use carlos_proto::task_overview::{TaskMainState, TaskOverviewData, TaskOverviewEntry};
use carlos_proto::tasks::{TaskField, TaskKind};

use super::tasks_details::Details;

pub(super) fn detail(details: &Details, kind: TaskKind) -> Vec<String> {
    let field = |field| details.field(field).unwrap_or("unavailable");
    match kind {
        TaskKind::Agent => agent(details),
        TaskKind::Process => vec![
            format!("Process: {}", field(TaskField::NativeId)),
            format!("Thread: {}", field(TaskField::Owner)),
            format!("Item: {}", field(TaskField::ProcessItem)),
            format!("Command: {}", field(TaskField::Command)),
            format!("Directory: {}", field(TaskField::Directory)),
            format!(
                "OS PID: {} · CPU: {} · RSS KiB: {}",
                field(TaskField::OsPid),
                field(TaskField::CpuPercent),
                field(TaskField::RssKib)
            ),
            "Present in the native background-process list; x terminates only this process.".into(),
        ],
        TaskKind::Task => claude(details),
    }
}

fn agent(details: &Details) -> Vec<String> {
    let field = |field| details.field(field).unwrap_or("unavailable");
    let mut lines = vec![
        format!("Thread: {}", field(TaskField::NativeId)),
        format!("Role: {}", field(TaskField::Role)),
        format!("Parent: {}", field(TaskField::Parent)),
        format!("Native state: {}", field(TaskField::NativeState)),
        format!(
            "Direct input allowed: {} (this view is read-only)",
            field(TaskField::DirectInput)
        ),
    ];
    if let Some(turn) = details.field(TaskField::LastTurn) {
        lines.push(format!(
            "Last observed turn: {turn} ({})",
            field(TaskField::LastTurnState)
        ));
    }
    if let Some(report) = details.field(TaskField::Report) {
        lines.push(format!("Agent report ({}):", field(TaskField::ReportState)));
        lines.push(report.into());
        if details.value.as_ref().is_some_and(|value| {
            value.properties.iter().any(|property| {
                property.field == TaskField::Report
                    && !property.complete
                    && property.text.len() >= 65536
            })
        }) {
            lines.push("[Display limited; complete native record retained]".into());
        }
    }
    lines.push(
        "Idle means no active native work; it does not prove the agent completed its task.".into(),
    );
    lines
}

fn claude(details: &Details) -> Vec<String> {
    let value = details.value.as_ref().expect("selected typed task");
    let mut lines = vec![format!(
        "Task {} · {}",
        details.field(TaskField::NativeId).unwrap_or("unavailable"),
        value.task.state
    )];
    for (field, label) in [
        (TaskField::Description, "Description"),
        (TaskField::Summary, "Summary"),
        (TaskField::RunLabel, "Run"),
        (TaskField::NativeType, "Type"),
        (TaskField::Parent, "Parent"),
        (TaskField::ParentCaller, "Caller"),
        (TaskField::AgentType, "Agent"),
        (TaskField::OutputFile, "Output"),
        (TaskField::Error, "Error"),
        (TaskField::UsageTokens, "total tokens"),
        (TaskField::UsageTools, "tool uses"),
        (TaskField::UsageDurationMillis, "duration ms"),
    ] {
        if let Some(text) = details.field(field).filter(|text| !text.is_empty()) {
            lines.push(format!("{label}: {text}"));
        }
    }
    lines
}

pub(super) fn overview(entries: &[TaskOverviewEntry]) -> Vec<String> {
    let now = carlos_core::session::usage::unix_now();
    entries
        .iter()
        .filter_map(|entry| match &entry.data {
            TaskOverviewData::Main {
                state,
                thinking_tokens,
                transient,
            } => {
                let label = transient
                    .as_ref()
                    .map(|label| label.text.clone())
                    .or_else(|| thinking_tokens.map(|tokens| format!("Thinking ~{tokens} tokens")))
                    .or_else(|| match state {
                        TaskMainState::Idle => None,
                        TaskMainState::Running => Some("Running".into()),
                        TaskMainState::RequiresAction => Some("Requires action".into()),
                    });
                label.map(|label| format!("Main: {label}"))
            }
            TaskOverviewData::Quota {
                label,
                used_pct,
                resets_at,
            } => (*resets_at > now).then(|| {
                format!(
                    "Quota {}: {used_pct:.1}% used; resets in {:.1}h",
                    label.text,
                    (*resets_at - now) as f64 / 3600.0
                )
            }),
            TaskOverviewData::Prompt {
                label,
                consumed,
                steer,
            } => Some(format!(
                "Prompt {} · {} · {}",
                label.text,
                if *consumed { "running" } else { "accepted" },
                if *steer { "steer" } else { "queued" }
            )),
            TaskOverviewData::ProcessError { label, message } => Some(format!(
                "Background processes ({}): {}",
                label.text, message.text
            )),
        })
        .collect()
}
