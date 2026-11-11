//! Read selected-child items in explicit bounded windows and retain only its bounded live output.

use std::time::Duration;

use serde_json::{json, Value};

use super::codex_panel_state::MAX_BYTES;
use super::state::AppState;

/// Append older native items without pretending a bounded window contains every message.
pub(super) fn page(app: &mut AppState, result: &Value) -> Result<(), String> {
    let activity = app
        .codex
        .panel
        .activity
        .as_mut()
        .ok_or("No selected child read")?;
    if activity.started.elapsed() > Duration::from_secs(20) {
        return Err("Child read exceeded 20 seconds".into());
    }
    let data = result["data"]
        .as_array()
        .ok_or("Malformed child item page")?;
    let bytes = result.to_string().len();
    if activity.bytes.saturating_add(bytes) > MAX_BYTES || activity.items.len() + data.len() > 128 {
        return Err("Child view reached its 8 MiB/128-item bound; r reloads latest".into());
    }
    for entry in data {
        super::codex_panel_pages::valid_id(&entry["turnId"])?;
        super::codex_panel_pages::valid_id(&entry["item"]["id"])?;
        if !activity.items.iter().any(|old| same_item(old, entry)) {
            activity.items.push(entry.clone());
        }
    }
    activity.bytes += bytes;
    activity.cursor = super::codex_panel_pages::cursor(result)?;
    activity.more = !activity.cursor.is_null();
    if activity.more
        && !activity
            .cursors
            .insert(activity.cursor.as_str().expect("validated cursor").into())
    {
        return Err("Repeated child item cursor".into());
    }
    if activity.more && activity.items.len() < activity.until {
        let request = activity.request.clone().ok_or("Missing child request")?;
        let cursor = activity.cursor.clone();
        let request = super::codex_panel::continue_read(app, request, cursor);
        app.codex
            .panel
            .activity
            .as_mut()
            .expect("child view")
            .request = Some(request);
    } else {
        activity.request = None;
    }
    Ok(())
}

/// Capture only the selected verified child; no unbounded per-agent renderer or cursor exists.
pub(super) fn event(app: &mut AppState, method: &str, params: &Value) {
    let Some(activity) = app
        .codex
        .panel
        .activity
        .as_mut()
        .filter(|activity| params["threadId"] == activity.thread)
    else {
        return;
    };
    if matches!(method, "item/started" | "item/completed") {
        let entry = json!({"turnId":params["turnId"],"item":params["item"]});
        if super::codex_panel_pages::valid_id(&entry["item"]["id"]).is_err() {
            return;
        }
        let bytes = entry.to_string().len();
        if bytes > 64 * 1024 {
            activity.error =
                Some("Live child record exceeds 64 KiB; r reads complete native items".into());
            return;
        }
        if let Some(index) = activity.items.iter().position(|old| same_item(old, &entry)) {
            activity.items[index] = entry;
        } else {
            activity.items.insert(0, entry);
        }
        activity.items.truncate(128);
        activity.bytes = activity
            .items
            .iter()
            .map(|entry| entry.to_string().len())
            .sum();
        while activity.bytes > MAX_BYTES {
            activity.items.pop();
            activity.bytes = activity
                .items
                .iter()
                .map(|entry| entry.to_string().len())
                .sum();
            activity.more = true;
        }
        activity.tail.clear();
    } else if matches!(
        method,
        "item/agentMessage/delta"
            | "item/commandExecution/outputDelta"
            | "item/reasoning/textDelta"
            | "item/reasoning/summaryTextDelta"
    ) {
        if let Some(delta) = params["delta"].as_str() {
            let safe = super::claude_question_state::safe_text(delta);
            activity.tail.push_str(&safe);
            if activity.tail.len() > 16 * 1024 {
                let mut start = activity.tail.len() - 16 * 1024;
                while !activity.tail.is_char_boundary(start) {
                    start += 1;
                }
                activity.tail.drain(..start);
            }
        }
    }
}

fn same_item(left: &Value, right: &Value) -> bool {
    left["turnId"] == right["turnId"] && left["item"]["id"] == right["item"]["id"]
}
