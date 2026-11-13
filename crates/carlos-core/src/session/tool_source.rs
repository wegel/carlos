//! Identify a successful source read without coloring or wrapping its output.

use serde_json::Value;

use super::shell_read::single_source_path;
use super::tools::{first_i64_at_paths, first_string_at_paths, tool_command, tool_name};

/// Read a source path only from a successful read tool or a single read action.
pub fn source_path_from_item(item: &Value) -> Option<String> {
    if first_i64_at_paths(item, &[&["exitCode"], &["exit_code"]]).unwrap_or(0) != 0
        || item.get("isError").and_then(Value::as_bool) == Some(true)
        || item.get("is_error").and_then(Value::as_bool) == Some(true)
        || first_string_at_paths(item, &[&["stderr"], &["metadata", "stderr"]])
            .is_some_and(|text| !text.trim().is_empty())
    {
        return None;
    }
    if tool_name(item).is_some_and(|name| name.eq_ignore_ascii_case("read")) {
        return first_string_at_paths(
            item,
            &[
                &["input", "filePath"],
                &["input", "file_path"],
                &["input", "path"],
                &["filePath"],
                &["path"],
            ],
        );
    }
    if let Some(actions) = item.get("commandActions").and_then(Value::as_array) {
        if actions.len() > 1 {
            return None;
        }
        if let Some(action) = actions
            .first()
            .filter(|action| action.get("type").and_then(Value::as_str) == Some("read"))
        {
            if let Some(path) = action.get("path").and_then(Value::as_str) {
                return Some(path.to_string());
            }
        }
    }
    single_source_path(&tool_command(item)?)
}
