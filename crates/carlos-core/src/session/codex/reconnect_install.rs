//! Install complete native facts once, retaining local drafts without resending them.

use std::collections::HashMap;

use anyhow::Result;
use serde_json::{json, Value};

use super::goals::GoalUpdate;
use super::panel_scope::PanelScope;
use crate::codex_snapshot::CodexSnapshot;
use crate::protocol_params::{parse_model_list_page, parse_thread_runtime_settings};
use crate::session::codex::panel_state::VerifiedOwners;
use crate::session::item_changes::ItemChanges;
use crate::session::state::SessionState;

/// Return checked native facts and only the client's remaining redraw/editor effects.
pub struct ReconnectInstalled {
    pub history: ItemChanges,
    pub goal: GoalUpdate,
}

/// Validate all fallible fields before replacing transcript or native control snapshots.
pub(crate) fn install(
    session: &mut SessionState,
    mut snapshot: CodexSnapshot,
    active: Option<String>,
) -> Result<ReconnectInstalled> {
    let runtime = parse_thread_runtime_settings(&snapshot.runtime)?;
    let (models, _) = parse_model_list_page(&json!({"result":snapshot.catalog}).to_string())?;
    anyhow::ensure!(
        snapshot.thread["id"] == *session.thread_id(),
        "Snapshot belongs to another thread"
    );
    let streams = super::reconnect_streams::Streams::from_thread(&snapshot.thread)?;
    if let Some(goal) = &snapshot.goal {
        super::goal_receive::validate(session.thread_id(), &goal["goal"])
            .map_err(anyhow::Error::msg)?;
        anyhow::ensure!(
            goal.get("goal").is_some(),
            "Codex did not return goal state"
        );
    }
    let facts = PanelFacts {
        ownership: verified_owners(session, &snapshot.panel)?,
        scopes: process_scopes(&snapshot.panel)?,
    };
    session.set_available_models(models);
    let history = install_history_runtime(session, &snapshot, runtime)?;
    session.codex_connection.streams = streams;
    install_queue(session, &snapshot);
    install_panel(session, &snapshot.panel, facts);
    let goal = install_goal(session, snapshot.goal);
    if let Some(turn) = active {
        session.record_turn_started(&turn);
    }
    session.reopen_codex();
    session.install_native_history(&snapshot.thread, snapshot.older_history.take());
    Ok(ReconnectInstalled { history, goal })
}

fn install_goal(session: &mut SessionState, goal: Option<Value>) -> GoalUpdate {
    let update = GoalUpdate::default();
    session.begin_codex_goal_snapshot(goal.is_some());
    if let Some(goal) = goal {
        if goal["goal"].is_null() {
            super::goal_receive::observed(
                session,
                "thread/goal/cleared",
                &json!({"threadId":session.thread_id()}),
            );
        } else {
            super::goal_receive::observed(
                session,
                "thread/goal/updated",
                &json!({"threadId":session.thread_id(),"goal":goal["goal"]}),
            );
        }
    }
    update
}

fn verified_owners(session: &SessionState, panel: &Value) -> Result<VerifiedOwners> {
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
    VerifiedOwners::new(session.thread_id(), session.codex_scope().epoch(), owners)
        .map_err(anyhow::Error::msg)
}

fn install_history_runtime(
    session: &mut SessionState,
    snapshot: &CodexSnapshot,
    runtime: crate::protocol_params::ThreadRuntimeSettings,
) -> Result<ItemChanges> {
    session.set_active_turn_id(None);
    // `install` has already rejected a snapshot of another thread, so this cannot fail here.
    let history =
        crate::session::item_history::replace_history_from_thread(session, &snapshot.thread)?;
    session.set_context_usage(None);
    session.clear_rate_limits();
    session.set_runtime_settings(runtime.model, runtime.effort, runtime.summary);
    session.queue_runtime_settings(None, None, None);
    let future = session.codex_runtime_choices();
    session.install_codex_settings(future);
    session.install_codex_startup();
    Ok(history)
}

fn install_queue(session: &mut SessionState, snapshot: &CodexSnapshot) {
    session.install_codex_queue(snapshot.queue.as_ref());
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

fn install_panel(session: &mut SessionState, panel: &Value, facts: PanelFacts) {
    let scope = PanelScope {
        native: session.codex_scope(),
        generation: session.codex_connection.generation,
        rebuilding: session.codex_connection.rebuilding,
        visible: false,
        detail: false,
        selected: None,
    };
    session.install_codex_panel(panel, facts.ownership, facts.scopes, scope);
}
