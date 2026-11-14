//! Drive the bounded native task reader off-thread without constructing a session or view.

use std::time::{Duration, Instant};

use anyhow::Result;
use serde_json::{json, Value};

use super::panel_reader::PageStep;
use super::panel_state::Snapshot;
use crate::backend::BackendClient;
use crate::protocol_history::is_unsupported_method;
use crate::protocol_params::extract_result_object;

/// Read the existing bounded recent window without subscribing to or resuming children.
pub(crate) fn reconnect_panel(client: &dyn BackendClient, thread: &str) -> Result<Value> {
    let mut read = Snapshot::new(thread, 0, Value::Null, false);
    let deadline = read.started + Duration::from_secs(20);
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| anyhow::anyhow!("Codex reconnect panel exceeded20 seconds"))?;
        let line = client.call(
            &read.request.method,
            read.request.params.clone(),
            remaining.min(Duration::from_secs(5)),
        )?;
        let frame: Value = serde_json::from_str(&line)?;
        if frame.get("error").is_some_and(is_unsupported_method) {
            return Ok(json!({"unavailable":true}));
        }
        let step = read_response(&mut read, &line, &frame)?;
        match step {
            PageStep::Continue(cursor) => read.request.params["cursor"] = cursor,
            PageStep::Read { method, params } => {
                read.request.method = method.into();
                read.request.params = params;
            }
            PageStep::Complete => break,
        }
    }
    Ok(export(read))
}

fn read_response(read: &mut Snapshot, line: &str, frame: &Value) -> Result<PageStep> {
    if read.request.method == "thread/backgroundTerminals/list" && frame.get("error").is_some() {
        return Ok(read.process_failed(
            frame["error"]["message"]
                .as_str()
                .unwrap_or("Processes unavailable"),
        ));
    }
    let parsed = extract_result_object(line)?;
    read.page(&parsed["result"]).map_err(anyhow::Error::msg)
}

fn export(read: Snapshot) -> Value {
    let verified: Vec<_> = read.verified_owners().collect();
    let notice = read.notice();
    let error = (!read.errors.is_empty()).then(|| read.errors.join("\n"));
    json!({"agents":read.agents,"processes":read.processes.into_iter()
        .map(|process| json!({"thread":process.thread,"data":process.data})).collect::<Vec<_>>(),
        "older":read.older,"error":error,"notice":notice,"verifiedOwners":verified,
        "processScopes":read.process_scopes})
}
