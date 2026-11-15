//! Keep the existing constructor while the remaining lookup/history callers move into core.

use crate::app::state::AppState;
use carlos_core::backend::BackendKind;

pub(super) fn app(backend: BackendKind) -> AppState {
    let mut app = AppState::new("thread".into());
    app.session
        .set_codex_enabled_for_test(backend == BackendKind::Codex);
    app.session
        .set_claude_enabled_for_test(backend == BackendKind::Claude);
    app.session.replace_codex_item_turn(Some("turn".into()));
    app.session.set_summary_enabled_for_test(true);
    app
}
