//! Restore the root's live turn without treating a stale persisted turn as running.

use std::collections::{HashMap, VecDeque};

use anyhow::{ensure, Result};
use serde_json::Value;

use super::codex_reconnect_state::ReplayFrame;

/// Combine native live status with exact turn records and actual new-connection turn events.
pub(super) fn active_turn(
    thread: &Value,
    journal: &VecDeque<ReplayFrame>,
) -> Result<Option<String>> {
    let turns = thread["turns"].as_array().into_iter().flatten();
    let mut statuses = turns
        .filter_map(|turn| {
            Some((
                turn["id"].as_str()?.to_string(),
                turn["status"].as_str()?.to_string(),
            ))
        })
        .collect::<HashMap<_, _>>();
    let mut live = thread["status"]["type"] == "active";
    let mut active = if live {
        let running = statuses
            .iter()
            .filter(|(_, status)| *status == "inProgress")
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        ensure!(
            running.len() <= 1,
            "Native thread has multiple in-progress turns"
        );
        running.into_iter().next()
    } else {
        None
    };
    for frame in journal {
        let params = &frame.event["params"];
        if params["threadId"] != thread["id"] {
            continue;
        }
        update(&mut statuses, &mut active, &mut live, &frame.event);
    }
    ensure!(
        !live || active.is_some(),
        "Codex is active but has not identified its turn; retry reconnect to refresh"
    );
    Ok(active)
}

fn update(
    statuses: &mut HashMap<String, String>,
    active: &mut Option<String>,
    live: &mut bool,
    event: &Value,
) {
    let params = &event["params"];
    let Some(id) = params["turn"]["id"].as_str() else {
        return;
    };
    match event["method"].as_str() {
        Some("turn/started")
            if !statuses
                .get(id)
                .is_some_and(|status| status != "inProgress") =>
        {
            *active = Some(id.into());
            statuses.insert(id.into(), "inProgress".into());
            *live = true;
        }
        Some("turn/completed") => {
            statuses.insert(
                id.into(),
                params["turn"]["status"]
                    .as_str()
                    .unwrap_or("completed")
                    .into(),
            );
            if active.as_deref() == Some(id) {
                *active = None;
                *live = false;
            }
        }
        _ => {}
    }
}
