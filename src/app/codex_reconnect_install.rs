//! Install complete native facts once, retaining local drafts without resending them.

use std::collections::HashMap;

use anyhow::Result;
use serde_json::{json, Value};

use super::state::AppState;
use crate::codex_snapshot::CodexSnapshot;
use crate::protocol_params::{parse_model_list_page, parse_thread_runtime_settings};
use crate::session::codex::panel_state::VerifiedOwners;

/// Validate all fallible fields before replacing transcript or native control snapshots.
pub(super) fn install(
    app: &mut AppState,
    snapshot: CodexSnapshot,
    active: Option<String>,
) -> Result<()> {
    let runtime = parse_thread_runtime_settings(&snapshot.runtime)?;
    let (models, _) = parse_model_list_page(&json!({"result":snapshot.catalog}).to_string())?;
    anyhow::ensure!(
        snapshot.thread["id"] == *app.session.thread_id(),
        "Snapshot belongs to another thread"
    );
    let streams = super::codex_reconnect_streams::Streams::from_thread(&snapshot.thread)?;
    let editor = app.view.runtime_settings.show_model_settings.then(|| {
        json!({"model":app.model_settings_model_value(),
        "effort":app.model_settings_effort_value(),"summary":app.model_settings_summary_value()})
    });
    if let Some(goal) = &snapshot.goal {
        super::codex_goal_receive::validate(&app.session.thread_id(), &goal["goal"])
            .map_err(anyhow::Error::msg)?;
        anyhow::ensure!(
            goal.get("goal").is_some(),
            "Codex did not return goal state"
        );
    }
    let facts = PanelFacts {
        ownership: verified_owners(app, &snapshot.panel)?,
        scopes: process_scopes(&snapshot.panel)?,
    };
    app.set_available_models(models);
    install_history_runtime(app, &snapshot, runtime)?;
    app.codex.connection.streams = streams;
    if let Some(editor) = editor {
        app.view.codex_settings.rejected = Some(editor);
        super::codex_settings_draft::restore_editor(app);
    }
    install_queue(app, &snapshot);
    install_panel(app, &snapshot.panel, facts);
    install_goal(app, snapshot.goal);
    if let Some(turn) = active {
        app.record_turn_started(&turn);
    }
    app.codex.closed = false;
    Ok(())
}

fn install_goal(app: &mut AppState, goal: Option<Value>) {
    app.session.begin_codex_goal_snapshot(goal.is_some());
    if let Some(goal) = goal {
        if goal["goal"].is_null() {
            super::codex_goal_receive::observed(
                app,
                "thread/goal/cleared",
                &json!({"threadId":app.session.thread_id()}),
            );
        } else {
            super::codex_goal_receive::observed(
                app,
                "thread/goal/updated",
                &json!({"threadId":app.session.thread_id(),"goal":goal["goal"]}),
            );
        }
    }
}

fn verified_owners(app: &AppState, panel: &Value) -> Result<VerifiedOwners> {
    let owners = panel
        .get("verifiedOwners")
        .map(|value| {
            value
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Malformed verified task owners"))
        })
        .transpose()?
        .into_iter()
        .flatten()
        .map(|id| {
            id.as_str()
                .map(str::to_string)
                .ok_or_else(|| anyhow::anyhow!("Invalid verified task owner"))
        })
        .collect::<Result<Vec<_>>>()?;
    VerifiedOwners::new(&app.session.thread_id(), app.codex.epoch, owners)
        .map_err(anyhow::Error::msg)
}

fn install_history_runtime(
    app: &mut AppState,
    snapshot: &CodexSnapshot,
    runtime: crate::protocol_params::ThreadRuntimeSettings,
) -> Result<()> {
    app.session.set_active_turn_id(None);
    // `install` has already rejected a snapshot of another thread, so this cannot fail here.
    super::item_history::replace_history_from_thread(app, &snapshot.thread)?;
    app.session.set_context_usage(None);
    app.clear_rate_limits();
    app.set_runtime_settings(runtime.model, runtime.effort, runtime.summary);
    app.queue_runtime_settings(None, None, None);
    let future = super::codex_settings_draft::choices(app);
    app.session.install_codex_settings(future);
    Ok(())
}

fn install_queue(app: &mut AppState, snapshot: &CodexSnapshot) {
    app.session.install_codex_queue(snapshot.queue.as_ref());
    app.view.codex_queue.error = None;
}

struct PanelFacts {
    ownership: VerifiedOwners,
    scopes: HashMap<String, Option<String>>,
}

fn process_scopes(panel: &Value) -> Result<HashMap<String, Option<String>>> {
    let mut scopes = HashMap::new();
    if let Some(value) = panel.get("processScopes") {
        let values = value
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("Malformed background process scope results"))?;
        anyhow::ensure!(values.len() <= 257, "Too many background process scopes");
        for (owner, error) in values {
            anyhow::ensure!(
                !owner.is_empty() && owner.len() <= 4096,
                "Invalid background process owner"
            );
            let error = if error.is_null() {
                None
            } else {
                let text = error
                    .as_str()
                    .filter(|text| text.len() <= 1024)
                    .ok_or_else(|| anyhow::anyhow!("Malformed background process scope error"))?;
                Some(text.to_string())
            };
            scopes.insert(owner.clone(), error);
        }
    }
    Ok(scopes)
}

fn install_panel(app: &mut AppState, panel: &Value, facts: PanelFacts) {
    let scope = super::codex_panel::scope(&app.codex, &app.view.codex_panel);
    app.session
        .install_codex_panel(panel, facts.ownership, facts.scopes, scope);
    app.view.codex_panel.detail = false;
}
