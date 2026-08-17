//! Local Grok session listing from `~/.grok/sessions`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

use super::types::grok_sessions_root;

// --- Types ---

#[derive(Debug, Clone)]
pub(crate) struct GrokSessionSummary {
    pub(crate) session_id: String,
    pub(crate) title: Option<String>,
    pub(crate) cwd: String,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
}

// --- Public API ---

pub(crate) fn load_grok_thread_summaries(cwd: &Path) -> Result<Vec<GrokSessionSummary>> {
    let Some(root) = grok_sessions_root() else {
        return Ok(Vec::new());
    };
    load_grok_thread_summaries_from(root, cwd)
}

pub(crate) fn load_grok_thread_summaries_from(
    sessions_root: PathBuf,
    cwd: &Path,
) -> Result<Vec<GrokSessionSummary>> {
    if !sessions_root.is_dir() {
        return Ok(Vec::new());
    }
    let cwd = cwd.to_string_lossy();
    let mut out = Vec::new();
    for group in fs::read_dir(&sessions_root).with_context(|| {
        format!(
            "failed to read Grok sessions dir {}",
            sessions_root.display()
        )
    })? {
        let group = group?;
        if !group.file_type()?.is_dir() {
            continue;
        }
        collect_group_summaries(&group.path(), cwd.as_ref(), &mut out)?;
    }
    Ok(out)
}

pub(super) fn parse_grok_session_list(
    result: &Value,
    fallback_cwd: &str,
) -> Vec<GrokSessionSummary> {
    let Some(sessions) = result.get("sessions").and_then(Value::as_array) else {
        return Vec::new();
    };
    sessions
        .iter()
        .filter_map(|session| parse_listed_session(session, fallback_cwd))
        .collect()
}

// --- Disk scan ---

fn collect_group_summaries(
    group: &Path,
    cwd: &str,
    out: &mut Vec<GrokSessionSummary>,
) -> Result<()> {
    for entry in fs::read_dir(group)
        .with_context(|| format!("failed to read Grok session group {}", group.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let summary_path = entry.path().join("summary.json");
        if let Some(summary) = summarize_session_file(&summary_path, cwd) {
            out.push(summary);
        }
    }
    Ok(())
}

fn summarize_session_file(path: &Path, cwd: &str) -> Option<GrokSessionSummary> {
    let contents = fs::read_to_string(path).ok()?;
    let parsed: Value = serde_json::from_str(&contents).ok()?;
    let session_cwd = parsed
        .pointer("/info/cwd")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !session_cwd.is_empty() && session_cwd != cwd {
        return None;
    }
    let id = parsed
        .pointer("/info/id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            path.parent()
                .and_then(|parent| parent.file_name())
                .map(|name| name.to_string_lossy().into_owned())
        })?;
    let title = first_nonempty_str(&parsed, &["session_summary", "generated_title"]);
    let updated = parse_iso_timestamp_secs(
        parsed
            .get("updated_at")
            .or_else(|| parsed.get("last_active_at"))
            .and_then(Value::as_str)
            .unwrap_or(""),
    );
    let created = parse_iso_timestamp_secs(
        parsed
            .get("created_at")
            .and_then(Value::as_str)
            .unwrap_or(""),
    );
    Some(GrokSessionSummary {
        session_id: id,
        title,
        cwd: if session_cwd.is_empty() {
            cwd.to_string()
        } else {
            session_cwd.to_string()
        },
        created_at: created,
        updated_at: if updated == 0 { created } else { updated },
    })
}

fn parse_listed_session(session: &Value, fallback_cwd: &str) -> Option<GrokSessionSummary> {
    let id = session
        .get("sessionId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())?
        .to_string();
    let title = session
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(ToOwned::to_owned);
    let cwd = session
        .get("cwd")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|cwd| !cwd.is_empty())
        .unwrap_or(fallback_cwd)
        .to_string();
    let updated = parse_iso_timestamp_secs(
        session
            .get("updatedAt")
            .and_then(Value::as_str)
            .unwrap_or(""),
    );
    Some(GrokSessionSummary {
        session_id: id,
        title,
        cwd,
        created_at: updated,
        updated_at: updated,
    })
}

fn first_nonempty_str(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToOwned::to_owned)
    })
}

pub(crate) fn parse_iso_timestamp_secs(raw: &str) -> i64 {
    let date_time = raw.get(..19).unwrap_or("");
    let Some((date, time)) = date_time.split_once('T') else {
        return 0;
    };
    let mut date_parts = date.split('-');
    let Ok(year) = date_parts.next().unwrap_or("").parse::<i64>() else {
        return 0;
    };
    let Ok(month) = date_parts.next().unwrap_or("").parse::<i64>() else {
        return 0;
    };
    let Ok(day) = date_parts.next().unwrap_or("").parse::<i64>() else {
        return 0;
    };
    let mut time_parts = time.split(':');
    let Ok(hour) = time_parts.next().unwrap_or("").parse::<i64>() else {
        return 0;
    };
    let Ok(minute) = time_parts.next().unwrap_or("").parse::<i64>() else {
        return 0;
    };
    let Ok(second) = time_parts.next().unwrap_or("").parse::<i64>() else {
        return 0;
    };
    days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let mut year = year;
    let mut month = month;
    if month <= 2 {
        year -= 1;
        month += 9;
    } else {
        month -= 3;
    }
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let doy = (153 * month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
