//! Read selected-child items in explicit bounded windows and retain only its bounded live output.

use std::time::Duration;

use serde_json::{json, Value};

use super::panel_state::{Activity, MAX_BYTES};
use crate::session::state::SessionState;

/// Append older native items without pretending a bounded window contains every message.
pub fn page(session: &mut SessionState, thread: &str, result: &Value) -> Result<(), String> {
    if session
        .codex_panel
        .activities
        .get(thread)
        .and_then(|activity| activity.owner.as_ref())
        .is_some_and(|owner| !owner.available(session, &owner.context))
    {
        session.codex_panel.activities.remove(thread);
        return Ok(());
    }
    let other_bytes = retained_bytes(session, Some(thread));
    let activity = session
        .codex_panel
        .activities
        .get_mut(thread)
        .ok_or("No addressed child read")?;
    accept_page(activity, result, other_bytes)?;
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
        if let Some(owner) = &mut activity.owner {
            owner.known = true;
        }
        activity.changed(true);
    }
    Ok(())
}

fn accept_page(
    activity: &mut super::panel_state::Activity,
    result: &Value,
    other_bytes: usize,
) -> Result<(), String> {
    if activity.started.elapsed() > Duration::from_secs(20) {
        return Err("Child read exceeded 20 seconds".into());
    }
    let data = result["data"]
        .as_array()
        .ok_or("Malformed child item page")?;
    let bytes = crate::session::task_window_bytes::encoded_bytes(result);
    if other_bytes
        .saturating_add(activity.bytes)
        .saturating_add(activity.tail.len())
        .saturating_add(bytes)
        > MAX_BYTES
        || activity.items.len() + data.len() > 128
    {
        return Err("Child view reached its 8 MiB/128-item bound; r reloads latest".into());
    }
    let cursor = super::panel_pages::cursor(result)?;
    if !cursor.is_null()
        && activity
            .cursors
            .contains(cursor.as_str().expect("validated cursor"))
    {
        return Err("Repeated child item cursor".into());
    }
    for entry in data {
        super::panel_pages::valid_id(&entry["turnId"])?;
        super::panel_pages::valid_id(&entry["item"]["id"])?;
    }
    for entry in data {
        if !activity.items.iter().any(|old| same_item(old, entry)) {
            activity.items.push(entry.clone());
        }
    }
    activity.bytes += bytes;
    activity.cursor = cursor;
    activity.more = !activity.cursor.is_null();
    if activity.more
        && !activity
            .cursors
            .insert(activity.cursor.as_str().expect("validated cursor").into())
    {
        return Err("Repeated child item cursor".into());
    }
    Ok(())
}

/// Capture only the selected verified child; no unbounded per-agent renderer or cursor exists.
pub fn event(session: &mut SessionState, method: &str, params: &Value) {
    let Some(thread) = params["threadId"].as_str() else {
        return;
    };
    let scope = crate::session::task_window_state::Scope::capture(session);
    let legacy_known = session
        .codex_panel
        .agents
        .iter()
        .any(|agent| agent["id"] == thread);
    let keys: Vec<_> = session
        .codex_panel
        .activities
        .iter()
        .filter(|(_, activity)| {
            activity.thread == thread
                && activity.owner.as_ref().map_or(legacy_known, |owner| {
                    !owner.retired
                        && owner.scope == scope
                        && owner
                            .context
                            .reader
                            .load(std::sync::atomic::Ordering::Acquire)
                })
        })
        .map(|(key, _)| key.clone())
        .collect();
    let delta = matches!(
        method,
        "item/agentMessage/delta"
            | "item/commandExecution/outputDelta"
            | "item/reasoning/textDelta"
            | "item/reasoning/summaryTextDelta"
    )
    .then(|| {
        params["delta"]
            .as_str()
            .map(crate::session::text::safe_text)
    })
    .flatten();
    for key in keys {
        let other_bytes = retained_bytes(session, Some(&key));
        let activity = session
            .codex_panel
            .activities
            .get_mut(&key)
            .expect("interested child");
        apply_event(activity, method, params, delta.as_deref(), other_bytes);
    }
}

fn apply_event(
    activity: &mut Activity,
    method: &str,
    params: &Value,
    delta: Option<&str>,
    other_bytes: usize,
) {
    if matches!(method, "item/started" | "item/completed") {
        let entry = json!({"turnId":params["turnId"],"item":params["item"]});
        if super::panel_pages::valid_id(&entry["item"]["id"]).is_err() {
            return;
        }
        store_item(activity, entry, MAX_BYTES.saturating_sub(other_bytes));
        activity.changed(true);
    } else if let Some(delta) = delta {
        let cap = (16 * 1024).min(
            MAX_BYTES
                .saturating_sub(other_bytes)
                .saturating_sub(activity.bytes),
        );
        append_tail(&mut activity.tail, delta, cap);
        activity.changed(false);
    }
}

fn append_tail(tail: &mut String, delta: &str, cap: usize) {
    if delta.len() >= cap {
        tail.clear();
    }
    let mut start = delta.len().saturating_sub(cap);
    while !delta.is_char_boundary(start) {
        start += 1;
    }
    tail.push_str(&delta[start..]);
    let mut start = tail.len().saturating_sub(cap);
    while !tail.is_char_boundary(start) {
        start += 1;
    }
    tail.drain(..start);
}

fn same_item(left: &Value, right: &Value) -> bool {
    left["turnId"] == right["turnId"] && left["item"]["id"] == right["item"]["id"]
}

fn store_item(activity: &mut super::panel_state::Activity, entry: Value, budget: usize) {
    let bytes = crate::session::task_window_bytes::encoded_bytes(&entry);
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
        .map(crate::session::task_window_bytes::encoded_bytes)
        .sum();
    while activity.bytes > budget {
        activity.items.pop();
        activity.bytes = activity
            .items
            .iter()
            .map(crate::session::task_window_bytes::encoded_bytes)
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
