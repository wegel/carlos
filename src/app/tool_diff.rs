//! Diff-block extraction and heuristic detection from tool output JSON.

use std::collections::HashSet;

use serde_json::Value;

use super::DiffBlock;

pub(super) fn is_probably_diff_text(text: &str) -> bool {
    let t = text.trim();
    t.starts_with("diff --git ")
        || t.starts_with("@@ ")
        || (t.contains("\n@@ ") && (t.contains("\n+++ ") || t.contains("\n--- ")))
        || (t.contains('\n') && t.contains("\n+++ ") && t.contains("\n--- "))
}

pub(super) fn infer_file_path_from_object(obj: &serde_json::Map<String, Value>) -> Option<String> {
    for key in ["filePath", "path", "file", "filename"] {
        if let Some(v) = obj.get(key).and_then(Value::as_str) {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

pub(super) fn collect_diff_blocks_recursive(
    value: &Value,
    current_file_path: Option<&str>,
    seen: &mut HashSet<String>,
    out: &mut Vec<DiffBlock>,
) {
    match value {
        Value::Object(obj) => {
            let inferred_path = infer_file_path_from_object(obj);
            let local_path = inferred_path.as_deref().or(current_file_path);

            if let Some(diff) = obj.get("diff").and_then(Value::as_str) {
                if !diff.is_empty() && is_probably_diff_text(diff) {
                    let key = format!("{}::{}", local_path.unwrap_or(""), diff);
                    if seen.insert(key) {
                        out.push(DiffBlock {
                            file_path: local_path.map(ToOwned::to_owned),
                            diff: diff.to_string(),
                        });
                    }
                }
            }

            for nested in obj.values() {
                collect_diff_blocks_recursive(nested, local_path, seen, out);
            }
        }
        Value::Array(arr) => {
            for nested in arr {
                collect_diff_blocks_recursive(nested, current_file_path, seen, out);
            }
        }
        _ => {}
    }
}

pub(super) fn extract_diff_blocks(item: &Value) -> Vec<DiffBlock> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    collect_diff_blocks_recursive(item, None, &mut seen, &mut out);
    out
}

pub(super) fn command_execution_diff_output(item: &Value) -> Option<String> {
    if item.get("type").and_then(Value::as_str) != Some("commandExecution") {
        return None;
    }
    let candidates = [
        item.get("aggregatedOutput").and_then(Value::as_str),
        item.get("formattedOutput").and_then(Value::as_str),
        item.get("stdout").and_then(Value::as_str),
        item.get("output").and_then(Value::as_str),
    ];
    for c in candidates.into_iter().flatten() {
        if !c.trim().is_empty() && is_probably_diff_text(c) {
            return Some(c.to_string());
        }
    }
    None
}
