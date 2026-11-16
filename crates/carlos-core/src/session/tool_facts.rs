//! Cache observed native tool kind, outcome and result size beside the original body.

use carlos_proto::session::{ToolKind, ToolStatus};
use serde_json::Value;

use super::models::{Message, MessageKind, Role, ToolFacts};
use super::tool_diff_summary::diff_counts;
use super::tools::{
    command_execution_diff_output, first_i64_at_paths, tool_command, tool_name, value_at_path,
};

pub(super) fn collect(message: &Message, item: &Value) -> ToolFacts {
    let previous = message.tool_facts.as_deref();
    let kind = tool_kind(message, item);
    let kind = if kind == ToolKind::Other && item["kind"].as_str().is_none() {
        previous.map_or(kind, |facts| facts.kind)
    } else {
        kind
    };
    let status = observed_status(item)
        .or_else(|| {
            previous
                .map(|facts| facts.status)
                .filter(|status| *status != ToolStatus::Running)
        })
        .unwrap_or(ToolStatus::Unknown);
    let (added, removed) = if message.kind == MessageKind::Diff {
        let (added, removed) = diff_counts(&message.text);
        (Some(added), Some(removed))
    } else {
        (None, None)
    };
    let summary = (message.role == Role::ToolOutput && !message.text.is_empty()).then(|| {
        let lines = result_text(item).unwrap_or(&message.text).lines().count();
        format!("{lines} {}", if lines == 1 { "line" } else { "lines" })
    });
    ToolFacts {
        kind,
        status,
        summary,
        added,
        removed,
    }
}

fn result_text(item: &Value) -> Option<&str> {
    [
        &["aggregatedOutput"][..],
        &["aggregated_output"],
        &["output"],
        &["text"],
        &["result"],
        &["metadata", "output"],
        &["state", "output"],
    ]
    .into_iter()
    .find_map(|path| value_at_path(item, path)?.as_str())
}

fn tool_kind(message: &Message, item: &Value) -> ToolKind {
    if message.kind == MessageKind::Diff {
        return ToolKind::Edit;
    }
    if let Some(kind) = item["kind"].as_str() {
        return match kind {
            "read" => ToolKind::Read,
            "search" => ToolKind::Search,
            "edit" | "delete" | "move" => ToolKind::Edit,
            "execute" => ToolKind::Command,
            _ => ToolKind::Other,
        };
    }
    let name = tool_name(item).unwrap_or_default().to_ascii_lowercase();
    match name.as_str() {
        "read" | "read_file" => return ToolKind::Read,
        "grep" | "rg" | "search" | "glob" | "find" | "codesearch" | "semeja" => {
            return ToolKind::Search;
        }
        "write" | "edit" | "applypatch" | "apply_patch" | "replace" => return ToolKind::Edit,
        "bash" | "shell" | "exec_command" => return ToolKind::Command,
        _ => {}
    }
    if message.file_path.is_some() {
        return ToolKind::Read;
    }
    if item["type"] == "commandExecution" || tool_command(item).is_some() {
        return ToolKind::Command;
    }
    ToolKind::Other
}

fn observed_status(item: &Value) -> Option<ToolStatus> {
    match item["status"].as_str() {
        Some("pending" | "in_progress" | "inProgress") => return Some(ToolStatus::Running),
        Some("interrupted" | "cancelled" | "canceled") => return Some(ToolStatus::Interrupted),
        Some("failed" | "declined") => return Some(ToolStatus::Failed),
        _ => {}
    }
    if item["interrupted"] == true {
        return Some(ToolStatus::Interrupted);
    }
    let exit = first_i64_at_paths(
        item,
        &[
            &["exitCode"],
            &["exit_code"],
            &["metadata", "exitCode"],
            &["metadata", "exit_code"],
            &["state", "exitCode"],
        ],
    );
    let error = item["isError"]
        .as_bool()
        .or_else(|| item["is_error"].as_bool());
    if error == Some(true) {
        return Some(ToolStatus::Failed);
    }
    if let Some(exit) = exit {
        let diff_exit = exit == 1 && command_execution_diff_output(item).is_some();
        return Some(if exit == 0 || diff_exit {
            ToolStatus::Ok
        } else {
            ToolStatus::Failed
        });
    }
    error.map(|_| ToolStatus::Ok)
}
