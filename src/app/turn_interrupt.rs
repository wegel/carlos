//! Apply an interrupt result and keep Escape bursts out of the local rewind chord.

use std::time::Instant;

use carlos_core::backend::{BackendClient, BackendKind};

use super::state::AppState;

pub(super) fn interrupt_active_turn(client: &dyn BackendClient, app: &mut AppState) {
    let requested = app.session.active_turn_id().is_some()
        || client.kind() == BackendKind::Claude && app.session.claude().activity().is_running();
    let changes = app.session.interrupt_active_turn(client);
    app.apply_item_changes(changes);
    if requested {
        app.suppress_esc_chord_after_interrupt(Instant::now());
    }
}
