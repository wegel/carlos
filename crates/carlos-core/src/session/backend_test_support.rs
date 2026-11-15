//! Drive real receiver fixtures through the production session API without a terminal.

use serde_json::Value;

use super::actions::ServerRequestAction;
use super::backend_update::{BackendChange, BackendContext, BackendUpdate};
use super::state::SessionState;
use crate::backend::BackendClient;
use crate::claude_backend::FixtureNormalizer;

/// Supply no client selection, draft, editor or consent token to the session.
pub(super) fn context(session: &SessionState) -> BackendContext<'static> {
    session.backend_context()
}

/// Apply normalized fan-out in full before executing its ordered backend actions.
pub(super) fn receive(session: &mut SessionState, client: &dyn BackendClient, line: &str) {
    let lines = client
        .handle_event(line)
        .unwrap_or_else(|| vec![line.to_string()]);
    let actions: Vec<_> = lines
        .iter()
        .filter_map(|line| {
            let update = session.handle_backend_line(line, &context(session));
            action(update)
        })
        .collect();
    for request in actions {
        let update = session.run_backend_action(client, request, &context(session));
        assert!(
            action(update).is_none(),
            "backend actions do not recursively invent actions"
        );
    }
    client.acknowledge_event(line.len());
}

/// Consume local changes without a view and preserve the old nested-action discard rule.
pub(super) fn action(update: BackendUpdate) -> Option<ServerRequestAction> {
    match update.change {
        BackendChange::Common(update) => update.action,
        BackendChange::Codex(update) => update.action,
        BackendChange::Claude(update) => update.action,
        BackendChange::None
        | BackendChange::Closed(_)
        | BackendChange::Items(_)
        | BackendChange::Installed(_) => None,
    }
}

/// Normalize fixture frames at the actual backend boundary, collecting every ordered Core action.
pub(super) fn normalize(
    core: &mut SessionState,
    native: &mut FixtureNormalizer,
    parsed: &Value,
) -> Vec<ServerRequestAction> {
    native_fixture_lines(core, native, parsed)
        .into_iter()
        .filter_map(|line| {
            let update = core.handle_backend_line(&line, &context(core));
            action(update)
        })
        .collect()
}

/// Keep actual private submission effects when a native fixture returns recoverable text.
pub(super) fn normalize_updates(
    core: &mut SessionState,
    native: &mut FixtureNormalizer,
    parsed: &Value,
) -> Vec<BackendUpdate> {
    native_fixture_lines(core, native, parsed)
        .into_iter()
        .map(|line| core.handle_backend_line(&line, &context(core)))
        .collect()
}

fn native_fixture_lines(
    core: &SessionState,
    native: &mut FixtureNormalizer,
    parsed: &Value,
) -> Vec<String> {
    let generation = parsed["params"]["generation"]
        .as_u64()
        .unwrap_or(core.claude().generation());
    if parsed["method"] == "claude/sessionChanged" && generation > core.claude().generation() {
        *native = Default::default();
    }
    if parsed["method"] == "claude/controller" && generation != core.claude().generation() {
        return Vec::new();
    }
    native.lines(parsed, core.thread_id(), generation)
}
