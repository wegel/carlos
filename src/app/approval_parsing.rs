//! Parsing of server approval requests into [`PendingApprovalRequest`] values.

use serde_json::json;
use serde_json::Value;

use super::state::{ApprovalRequestKind, PendingApprovalRequest};

// --- Entry Point ---

pub(super) fn pending_approval_from_request(
    method: &str,
    params: &serde_json::Map<String, Value>,
    request_id: Value,
) -> Option<PendingApprovalRequest> {
    match method {
        "item/commandExecution/requestApproval" => {
            approval_command_execution(method, params, request_id)
        }
        "item/fileChange/requestApproval" => {
            approval_file_change(method, params, request_id)
        }
        "item/permissions/requestApproval" => {
            approval_permissions(method, params, request_id)
        }
        "execCommandApproval" => approval_legacy_exec(method, params, request_id),
        "applyPatchApproval" => approval_legacy_patch(method, params, request_id),
        "claude/exitPlan/requestApproval" => {
            approval_claude_exit_plan(method, params, request_id)
        }
        _ => None,
    }
}

// --- Per-Method Helpers ---

fn approval_command_execution(
    method: &str,
    params: &serde_json::Map<String, Value>,
    request_id: Value,
) -> Option<PendingApprovalRequest> {
    let command = trimmed_str_param(params, "command").unwrap_or("command");
    let mut detail_lines = vec![command.to_string()];
    push_reason_and_cwd(params, &mut detail_lines);
    if let Some(extra) = params.get("additionalPermissions") {
        summarize_permission_profile(extra, &mut detail_lines);
    }
    let (can_accept_for_session, can_decline, can_cancel) = approval_decisions(params);
    Some(PendingApprovalRequest {
        request_id,
        method: method.to_string(),
        kind: ApprovalRequestKind::CommandExecution,
        title: "Approve command execution".to_string(),
        detail_lines,
        requested_permissions: None,
        can_accept_for_session,
        can_decline,
        can_cancel,
    })
}

fn approval_file_change(
    method: &str,
    params: &serde_json::Map<String, Value>,
    request_id: Value,
) -> Option<PendingApprovalRequest> {
    let mut detail_lines = Vec::new();
    if let Some(reason) = trimmed_str_param(params, "reason") {
        detail_lines.push(reason.to_string());
    }
    if let Some(root) = trimmed_str_param(params, "grantRoot") {
        detail_lines.push(format!("grant root: {root}"));
    }
    Some(PendingApprovalRequest {
        request_id,
        method: method.to_string(),
        kind: ApprovalRequestKind::FileChange,
        title: "Approve file changes".to_string(),
        detail_lines,
        requested_permissions: None,
        can_accept_for_session: true,
        can_decline: true,
        can_cancel: true,
    })
}

fn approval_permissions(
    method: &str,
    params: &serde_json::Map<String, Value>,
    request_id: Value,
) -> Option<PendingApprovalRequest> {
    let permissions = params
        .get("permissions")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mut detail_lines = Vec::new();
    if let Some(reason) = trimmed_str_param(params, "reason") {
        detail_lines.push(reason.to_string());
    }
    summarize_permission_profile(&permissions, &mut detail_lines);
    Some(PendingApprovalRequest {
        request_id,
        method: method.to_string(),
        kind: ApprovalRequestKind::Permissions,
        title: "Grant additional permissions".to_string(),
        detail_lines,
        requested_permissions: Some(permissions),
        can_accept_for_session: true,
        can_decline: true,
        can_cancel: false,
    })
}

fn approval_legacy_exec(
    method: &str,
    params: &serde_json::Map<String, Value>,
    request_id: Value,
) -> Option<PendingApprovalRequest> {
    let command = trimmed_str_param(params, "command").unwrap_or("command");
    let mut detail_lines = vec![command.to_string()];
    push_reason_and_cwd(params, &mut detail_lines);
    Some(PendingApprovalRequest {
        request_id,
        method: method.to_string(),
        kind: ApprovalRequestKind::LegacyExecCommand,
        title: "Approve command execution".to_string(),
        detail_lines,
        requested_permissions: None,
        can_accept_for_session: true,
        can_decline: true,
        can_cancel: true,
    })
}

fn approval_legacy_patch(
    method: &str,
    params: &serde_json::Map<String, Value>,
    request_id: Value,
) -> Option<PendingApprovalRequest> {
    let mut detail_lines = Vec::new();
    if let Some(reason) = trimmed_str_param(params, "reason") {
        detail_lines.push(reason.to_string());
    }
    if let Some(root) = trimmed_str_param(params, "grantRoot") {
        detail_lines.push(format!("grant root: {root}"));
    }
    Some(PendingApprovalRequest {
        request_id,
        method: method.to_string(),
        kind: ApprovalRequestKind::LegacyApplyPatch,
        title: "Approve patch application".to_string(),
        detail_lines,
        requested_permissions: None,
        can_accept_for_session: true,
        can_decline: true,
        can_cancel: true,
    })
}

fn approval_claude_exit_plan(
    method: &str,
    params: &serde_json::Map<String, Value>,
    request_id: Value,
) -> Option<PendingApprovalRequest> {
    let mut detail_lines =
        vec!["Claude wants to exit plan mode and continue with this plan.".to_string()];
    if let Some(path) = trimmed_str_param(params, "planFilePath") {
        detail_lines.push(format!("plan file: {path}"));
    }
    if let Some(plan) = trimmed_str_param(params, "plan") {
        detail_lines.push(String::new());
        detail_lines.extend(plan.lines().map(ToOwned::to_owned));
    }
    append_allowed_prompt_lines(params, &mut detail_lines);
    Some(PendingApprovalRequest {
        request_id,
        method: method.to_string(),
        kind: ApprovalRequestKind::ClaudeExitPlanMode,
        title: "Approve Claude plan".to_string(),
        detail_lines,
        requested_permissions: None,
        can_accept_for_session: false,
        can_decline: true,
        can_cancel: false,
    })
}

// --- Private Helpers ---

fn trimmed_str_param<'a>(params: &'a serde_json::Map<String, Value>, key: &str) -> Option<&'a str> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn push_reason_and_cwd(params: &serde_json::Map<String, Value>, lines: &mut Vec<String>) {
    if let Some(cwd) = trimmed_str_param(params, "cwd") {
        lines.push(format!("cwd: {cwd}"));
    }
    if let Some(reason) = trimmed_str_param(params, "reason") {
        lines.push(format!("reason: {reason}"));
    }
}

fn append_allowed_prompt_lines(
    params: &serde_json::Map<String, Value>,
    detail_lines: &mut Vec<String>,
) {
    let Some(allowed_prompts) = params.get("allowedPrompts").and_then(Value::as_array) else {
        return;
    };
    let mut lines = Vec::new();
    for prompt in allowed_prompts {
        let Some(obj) = prompt.as_object() else {
            continue;
        };
        let Some(text) = obj
            .get("prompt")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let tool = obj
            .get("tool")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        lines.push(match tool {
            Some(t) => format!("allowed: {text} ({t})"),
            None => format!("allowed: {text}"),
        });
    }
    if !lines.is_empty() {
        detail_lines.push(String::new());
        detail_lines.extend(lines);
    }
}

fn approval_decisions(params: &serde_json::Map<String, Value>) -> (bool, bool, bool) {
    let Some(decisions) = params.get("availableDecisions").and_then(Value::as_array) else {
        return (true, true, true);
    };

    let mut allow_session = false;
    let mut allow_decline = false;
    let mut allow_cancel = false;
    for entry in decisions {
        match entry.as_str() {
            Some("acceptForSession") => allow_session = true,
            Some("decline") => allow_decline = true,
            Some("cancel") => allow_cancel = true,
            _ => {}
        }
    }
    (allow_session, allow_decline, allow_cancel)
}

fn summarize_permission_profile(profile: &Value, out: &mut Vec<String>) {
    if let Some(read) = profile
        .get("fileSystem")
        .and_then(|fs| fs.get("read"))
        .and_then(Value::as_array)
    {
        let items: Vec<&str> = read.iter().filter_map(Value::as_str).collect();
        if !items.is_empty() {
            out.push(format!("read: {}", items.join(", ")));
        }
    }
    if let Some(write) = profile
        .get("fileSystem")
        .and_then(|fs| fs.get("write"))
        .and_then(Value::as_array)
    {
        let items: Vec<&str> = write.iter().filter_map(Value::as_str).collect();
        if !items.is_empty() {
            out.push(format!("write: {}", items.join(", ")));
        }
    }
    if profile
        .get("network")
        .and_then(|n| n.get("enabled"))
        .and_then(Value::as_bool)
        == Some(true)
    {
        out.push("network access".to_string());
    }
    if profile
        .get("macos")
        .and_then(|m| m.get("accessibility"))
        .and_then(Value::as_bool)
        == Some(true)
    {
        out.push("macOS accessibility".to_string());
    }
    if let Some(pref) = profile
        .get("macos")
        .and_then(|m| m.get("preferences"))
        .and_then(Value::as_str)
        .filter(|value| *value != "none")
    {
        out.push(format!("macOS preferences: {pref}"));
    }
}
