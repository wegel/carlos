//! Fork a Codex thread before the edited turn, with support for older servers.

use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::backend::{BackendClient, RewindForkRequest};
use crate::protocol_history::load_thread_history;
use crate::protocol_params::{
    extract_result_object, params_thread_fork, params_thread_rollback,
    parse_thread_id_from_start_or_resume,
};

/// Create and validate the retained prefix before submitting an edited prompt.
pub(crate) fn fork_before_edited_turn(
    client: &dyn BackendClient,
    thread_id: &str,
    request: RewindForkRequest,
    timeout: Duration,
) -> Result<String> {
    let mut params = params_thread_fork(thread_id);
    if let Some(before_turn_id) = &request.before_turn_id {
        params["beforeTurnId"] = Value::String(before_turn_id.clone());
    }
    let response = client.call("thread/fork", params, timeout)?;
    let fork_id = parse_thread_id_from_start_or_resume(&response)?;
    anyhow::ensure!(
        fork_id != thread_id,
        "app-server did not create a new thread"
    );
    let response = load_thread_history(client, &response, timeout)?;
    let drop_turns = turns_to_drop(&response, &request)?;
    if drop_turns == 0 {
        return Ok(response);
    }
    // Older servers ignore beforeTurnId. Roll back only the new fork, never the source.
    let rolled_back = client.call(
        "thread/rollback",
        params_thread_rollback(&fork_id, drop_turns),
        timeout,
    )?;
    let result = load_thread_history(client, &rolled_back, timeout)?;
    if request.before_turn_id.is_some() {
        anyhow::ensure!(
            turns_to_drop(&result, &request)? == 0,
            "edited turn remains in fork"
        );
    }
    Ok(result)
}

fn turns_to_drop(response: &str, request: &RewindForkRequest) -> Result<usize> {
    let Some(before_turn_id) = &request.before_turn_id else {
        return Ok(request.drop_turns);
    };
    let parsed = extract_result_object(response)?;
    let turns = parsed["result"]["thread"]["turns"]
        .as_array()
        .context("missing fork turn history")?;
    Ok(turns
        .iter()
        .position(|turn| turn.get("id").and_then(Value::as_str) == Some(before_turn_id))
        .map_or(0, |index| turns.len() - index))
}
