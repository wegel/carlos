//! Forward permission requests to the owner without choosing a native option.

use anyhow::Result;
use serde_json::{json, Value};

use super::process_writer::InputWriter;
use crate::protocol_events::EventQueue;

pub(super) fn receive_agent_request(
    request: Value,
    bytes: usize,
    epoch: u64,
    retired: bool,
    stdin: &InputWriter,
    events: &EventQueue,
) -> Result<()> {
    let callbacks = stdin.callbacks();
    let registered = callbacks.register(&request, bytes, epoch)?;
    if let Some(token) = registered.retired {
        events.send(
            json!({
                "method":"carlos/grok/callback-retired", "params":{"token":token}
            })
            .to_string(),
        )?;
    }
    if request["method"] == "session/request_permission"
        && !request["params"]["sessionId"]
            .as_str()
            .is_some_and(|id| !id.is_empty() && id.len() <= 4096)
    {
        let callback = callbacks.get(&json!(registered.token))?;
        return stdin.enqueue_callback(
            callback,
            json!({"code":-32602,"message":"invalid ACP permission session"}),
            true,
        );
    }
    if request["method"] == "session/request_permission" && retired {
        let callback = callbacks.get(&json!(registered.token))?;
        return stdin.enqueue_callback(callback, json!({"outcome":{"outcome":"cancelled"}}), false);
    }
    if request["method"] == "session/request_permission" {
        events.send(
            json!({
                "method":"carlos/grok/request",
                "params":{"token":registered.token, "request":request}
            })
            .to_string(),
        )?;
    } else {
        let callback = callbacks.get(&json!(registered.token))?;
        stdin.enqueue_callback(
            callback,
            json!({"code":-32601,"message":"unsupported ACP client method"}),
            true,
        )?;
    }
    Ok(())
}

pub(super) fn cancel_agent_requests(
    stdin: &InputWriter,
    events: &EventQueue,
    session: Option<&str>,
    epoch: Option<u64>,
) -> Result<()> {
    let (tokens, dispatched) = stdin.cancel_callbacks(session, epoch);
    retire_cards(events, tokens)?;
    dispatched
}

pub(super) fn retire_cards(events: &EventQueue, tokens: Vec<String>) -> Result<()> {
    for token in tokens {
        events.send(
            json!({
                "method":"carlos/grok/callback-retired", "params":{"token":token}
            })
            .to_string(),
        )?;
    }
    Ok(())
}
