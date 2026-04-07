//! Context-window token usage parsing and compact label formatting.

use serde_json::Value;

use super::text::visual_width;

// --- Usage Types ---
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ContextUsage {
    pub(super) used: u64,
    pub(super) max: u64,
}

// --- Usage Parsing ---
fn value_to_u64(v: &Value) -> Option<u64> {
    if let Some(n) = v.as_u64() {
        return Some(n);
    }
    if let Some(n) = v.as_i64() {
        return (n >= 0).then_some(n as u64);
    }
    None
}

fn select_context_used_tokens(total: Option<u64>, last: Option<u64>, max: u64) -> Option<u64> {
    match (total, last) {
        (Some(t), Some(l)) if t > max => Some(l),
        (Some(t), _) => Some(t),
        (None, Some(l)) => Some(l),
        (None, None) => None,
    }
}

pub(super) fn context_usage_from_thread_token_usage_params(
    params: &serde_json::Map<String, Value>,
) -> Option<ContextUsage> {
    let token_usage = params.get("tokenUsage")?.as_object()?;
    let max = token_usage
        .get("modelContextWindow")
        .and_then(value_to_u64)
        .filter(|n| *n > 0)?;

    let total_used = token_usage
        .get("total")
        .and_then(Value::as_object)
        .and_then(|t| t.get("totalTokens"))
        .and_then(value_to_u64);
    let last_used = token_usage
        .get("last")
        .and_then(Value::as_object)
        .and_then(|t| t.get("totalTokens"))
        .and_then(value_to_u64);
    let used = select_context_used_tokens(total_used, last_used, max)?;

    Some(ContextUsage {
        used: used.min(max),
        max,
    })
}

pub(super) fn context_usage_from_token_count_params(
    params: &serde_json::Map<String, Value>,
) -> Option<ContextUsage> {
    let info = params
        .get("msg")
        .and_then(Value::as_object)
        .and_then(|m| m.get("info"))
        .or_else(|| params.get("info"))?;
    let info_obj = info.as_object()?;

    let max = info_obj
        .get("model_context_window")
        .or_else(|| info_obj.get("modelContextWindow"))
        .and_then(value_to_u64)
        .filter(|n| *n > 0)?;

    let total_used = info_obj
        .get("total_token_usage")
        .and_then(Value::as_object)
        .and_then(|t| t.get("total_tokens"))
        .and_then(value_to_u64)
        .or_else(|| {
            info_obj
                .get("total")
                .and_then(Value::as_object)
                .and_then(|t| t.get("totalTokens").or_else(|| t.get("total_tokens")))
                .and_then(value_to_u64)
        });
    let last_used = info_obj
        .get("last_token_usage")
        .and_then(Value::as_object)
        .and_then(|t| t.get("total_tokens"))
        .and_then(value_to_u64)
        .or_else(|| {
            info_obj
                .get("last")
                .and_then(Value::as_object)
                .and_then(|t| t.get("totalTokens").or_else(|| t.get("total_tokens")))
                .and_then(value_to_u64)
        });
    let used = select_context_used_tokens(total_used, last_used, max)?;

    Some(ContextUsage {
        used: used.min(max),
        max,
    })
}

// --- Label Formatting ---
fn context_usage_compact_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{}m", n / 1_000_000)
    } else if n >= 1_000 {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

pub(super) fn context_usage_label(usage: ContextUsage) -> String {
    if usage.max == 0 {
        return String::new();
    }
    let pct = ((usage.used as f64 / usage.max as f64) * 100.0).round() as u64;
    format!(
        "{}/{} ({}%)",
        context_usage_compact_tokens(usage.used),
        context_usage_compact_tokens(usage.max),
        pct.min(100)
    )
}

pub(super) fn context_usage_placeholder_label() -> &'static str {
    "___k/___k (__%)"
}

pub(super) fn context_label_reserved_cells(context_label: Option<&str>) -> usize {
    let base = visual_width("999k/999k (99%)").max(visual_width(context_usage_placeholder_label()));
    let label_cells = context_label.map(visual_width).unwrap_or(0);
    base.max(label_cells)
}
