//! Read selected-child items in explicit bounded windows and retain only its bounded live output.

use std::time::Duration;

use serde_json::{json, Value};

use super::panel_state::MAX_BYTES;
use crate::session::state::SessionState;

/// Append older native items without pretending a bounded window contains every message.
pub fn page(session: &mut SessionState, thread: &str, result: &Value) -> Result<(), String> {
    let other_bytes = retained_bytes(session, Some(thread));
    let activity = session
        .codex_panel
        .activities
        .get_mut(thread)
        .ok_or("No addressed child read")?;
    if activity.started.elapsed() > Duration::from_secs(20) {
        return Err("Child read exceeded 20 seconds".into());
    }
    let data = result["data"]
        .as_array()
        .ok_or("Malformed child item page")?;
    let bytes = result.to_string().len();
    if other_bytes
        .saturating_add(activity.bytes)
        .saturating_add(activity.tail.len())
        .saturating_add(bytes)
        > MAX_BYTES
        || activity.items.len() + data.len() > 128
    {
        return Err("Child view reached its 8 MiB/128-item bound; r reloads latest".into());
    }
    for entry in data {
        super::panel_pages::valid_id(&entry["turnId"])?;
        super::panel_pages::valid_id(&entry["item"]["id"])?;
        if !activity.items.iter().any(|old| same_item(old, entry)) {
            activity.items.push(entry.clone());
        }
    }
    activity.bytes += bytes;
    activity.cursor = super::panel_pages::cursor(result)?;
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
        let request = super::panel::continue_read(session, request, cursor);
        session
            .codex_panel
            .activities
            .get_mut(thread)
            .expect("child view")
            .request = Some(request);
    } else {
        activity.request = None;
    }
    Ok(())
}

/// Capture only the selected verified child; no unbounded per-agent renderer or cursor exists.
pub fn event(session: &mut SessionState, method: &str, params: &Value) {
    let Some(thread) = params["threadId"].as_str() else {
        return;
    };
    let other_bytes = retained_bytes(session, Some(thread));
    let Some(activity) = session.codex_panel.activities.get_mut(thread) else {
        return;
    };
    if matches!(method, "item/started" | "item/completed") {
        let entry = json!({"turnId":params["turnId"],"item":params["item"]});
        if super::panel_pages::valid_id(&entry["item"]["id"]).is_err() {
            return;
        }
        store_item(activity, entry, MAX_BYTES.saturating_sub(other_bytes));
    } else if matches!(
        method,
        "item/agentMessage/delta"
            | "item/commandExecution/outputDelta"
            | "item/reasoning/textDelta"
            | "item/reasoning/summaryTextDelta"
    ) {
        if let Some(delta) = params["delta"].as_str() {
            let safe = crate::session::text::safe_text(delta);
            activity.tail.push_str(&safe);
            let cap = (16 * 1024).min(
                MAX_BYTES
                    .saturating_sub(other_bytes)
                    .saturating_sub(activity.bytes),
            );
            if activity.tail.len() > cap {
                let mut start = activity.tail.len() - cap;
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

fn store_item(activity: &mut super::panel_state::Activity, entry: Value, budget: usize) {
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
    while activity.bytes > budget {
        activity.items.pop();
        activity.bytes = activity
            .items
            .iter()
            .map(|entry| entry.to_string().len())
            .sum();
        activity.more = true;
    }
    activity.tail.clear();
}

fn retained_bytes(session: &SessionState, except: Option<&str>) -> usize {
    session
        .codex_panel
        .activities
        .iter()
        .filter(|(thread, _)| Some(thread.as_str()) != except)
        .fold(0usize, |sum, (_, activity)| {
            sum.saturating_add(activity.bytes)
                .saturating_add(activity.tail.len())
        })
}
