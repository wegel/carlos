//! Context-window token usage parsing and compact label formatting.

use carlos_core::session::context_usage::ContextUsage;

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
