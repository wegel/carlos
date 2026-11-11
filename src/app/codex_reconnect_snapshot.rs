//! Reuse the verified descendant reader while rebuilding off the UI thread.

use std::time::{Duration, Instant};

use anyhow::{ensure, Result};
use serde_json::{json, Value};

use super::state::AppState;
use crate::backend::BackendClient;
use crate::protocol_history::is_unsupported_method;
use crate::protocol_params::extract_result_object;

/// Read the existing bounded recent window without subscribing to or resuming children.
pub(crate) fn reconnect_panel(client: &dyn BackendClient, thread: &str) -> Result<Value> {
    let mut app = AppState::new(thread.into());
    let deadline = Instant::now() + Duration::from_secs(20);
    super::codex_panel::refresh(&mut app, false);
    while let Some(request) = app.codex.panel.outgoing.pop_front() {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| anyhow::anyhow!("Codex reconnect panel exceeded20 seconds"))?;
        let line = client.call(
            &request.method,
            request.params.clone(),
            remaining.min(Duration::from_secs(5)),
        )?;
        let frame: Value = serde_json::from_str(&line)?;
        if frame.get("error").is_some_and(is_unsupported_method) {
            return Ok(json!({"unavailable":true}));
        }
        if request.method == "thread/backgroundTerminals/list" && frame.get("error").is_some() {
            super::codex_panel_receive::failed(
                &mut app,
                &request,
                frame["error"]["message"]
                    .as_str()
                    .unwrap_or("Processes unavailable"),
            );
            continue;
        }
        let parsed = extract_result_object(&line)?;
        super::codex_panel_pages::page(&mut app, &parsed["result"]).map_err(anyhow::Error::msg)?;
    }
    ensure!(
        app.codex.panel.snapshot.is_none(),
        "Incomplete Codex reconnect panel"
    );
    let panel = app.codex.panel;
    Ok(
        json!({"agents":panel.agents,"processes":panel.processes.into_iter()
        .map(|process| json!({"thread":process.thread,"data":process.data})).collect::<Vec<_>>(),
        "older":panel.older,"error":panel.error,"notice":panel.notice,"verifiedOwners":panel.ownership.ids(),"processScopes":panel.process_scopes}),
    )
}
