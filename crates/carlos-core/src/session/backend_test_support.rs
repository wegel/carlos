//! Drive real receiver fixtures through the production session API without a terminal.

use super::actions::ServerRequestAction;
use super::backend_update::{BackendChange, BackendContext, BackendUpdate};
use super::state::SessionState;
use crate::backend::BackendClient;

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
