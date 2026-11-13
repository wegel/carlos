//! Reuse the verified descendant reader while rebuilding off the UI thread.

use std::time::{Duration, Instant};

use anyhow::{ensure, Result};
use serde_json::{json, Value};

use super::panel_scope::{PanelScope, PanelUpdate};
use super::scope::CodexScope;
use crate::backend::BackendClient;
use crate::protocol_history::is_unsupported_method;
use crate::protocol_params::extract_result_object;
use crate::session::state::SessionState;

/// Read the existing bounded recent window without subscribing to or resuming children.
pub(crate) fn reconnect_panel(client: &dyn BackendClient, thread: &str) -> Result<Value> {
    let mut session = SessionState::new(thread.into(), Default::default(), false);
    let scope = PanelScope {
        native: CodexScope {
            enabled: false,
            closed: false,
            epoch: 0,
        },
        generation: 0,
        rebuilding: false,
        visible: false,
        detail: false,
        selected: None,
    };
    let mut update = PanelUpdate::default();
    let deadline = Instant::now() + Duration::from_secs(20);
    super::panel::refresh(&mut session, scope, false);
    while let Some(request) = session.codex_panel.outgoing.pop_front() {
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
            super::panel_receive::failed(
                &mut session,
                scope,
                &mut update,
                &request,
                frame["error"]["message"]
                    .as_str()
                    .unwrap_or("Processes unavailable"),
            );
            continue;
        }
        let parsed = extract_result_object(&line)?;
        super::panel_pages::page(&mut session, scope, &mut update, &parsed["result"])
            .map_err(anyhow::Error::msg)?;
    }
    ensure!(
        session.codex_panel.snapshot.is_none(),
        "Incomplete Codex reconnect panel"
    );
    let panel = session.codex_panel;
    Ok(
        json!({"agents":panel.agents,"processes":panel.processes.into_iter()
        .map(|process| json!({"thread":process.thread,"data":process.data})).collect::<Vec<_>>(),
        "older":panel.older,"error":panel.error,"notice":panel.notice,"verifiedOwners":panel.ownership.ids(),"processScopes":panel.process_scopes}),
    )
}
