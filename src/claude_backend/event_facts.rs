//! Chooses plain Claude result and status facts before any client receives them.

use serde_json::{json, Value};

use super::event_normalizer::fact;

/// Classify compaction and context refresh without asking the app to interpret SDK status messages.
pub(super) fn status_fact(message: &Value) -> String {
    let started = message["status"] == "compacting";
    let compact =
        started || !message["compact_result"].is_null() || message["subtype"] == "compact_boundary";
    let label = message["status"].as_str().map(|status| {
        if status == "requesting" {
            "Requesting"
        } else {
            status
        }
    });
    fact(
        "claude/status",
        json!({"compaction":compact,"refreshContext":compact && !started,"label":label,
        "status":message["status"],"subtype":message["subtype"],"uuid":message["uuid"],
        "compact_result":message["compact_result"],"compact_error":message["compact_error"]}),
    )
}

/// Prefer native error details and retain a useful message when its error list is empty.
pub(super) fn result_error(message: &Value) -> String {
    message["errors"]
        .as_array()
        .map(|errors| {
            errors
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|error| !error.is_empty())
                .collect::<Vec<_>>()
                .join("; ")
        })
        .filter(|error| !error.is_empty())
        .or_else(|| {
            message["result"]
                .as_str()
                .map(str::trim)
                .filter(|error| !error.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Claude turn failed".to_string())
}
