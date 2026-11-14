//! Queue the next native task read and publish only complete windows for the captured root.

use std::time::Instant;

use serde_json::Value;

pub use super::panel_lineage::parent;
pub use super::panel_reader::cursor;
pub(crate) use super::panel_reader::valid_id;
use super::panel_reader::PageStep;
use super::panel_scope::{PanelScope, PanelUpdate};
use super::panel_state::Snapshot;
use crate::session::state::SessionState;

/// Apply one complete native page without installing a partial metadata snapshot.
pub fn page(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
    result: &Value,
) -> Result<(), String> {
    let step = session
        .codex_panel
        .snapshot
        .as_mut()
        .ok_or("No panel read")?
        .page(result)?;
    advance(session, scope, update, step);
    Ok(())
}

/// Retain a failed process scope and continue the same bounded read.
pub(crate) fn process_failed(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
    message: &str,
) {
    let step = session
        .codex_panel
        .snapshot
        .as_mut()
        .expect("panel read")
        .process_failed(message);
    advance(session, scope, update, step);
}

fn advance(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
    step: PageStep,
) {
    let request = match step {
        PageStep::Continue(cursor) => {
            let previous = session
                .codex_panel
                .snapshot
                .as_ref()
                .expect("panel read")
                .request
                .clone();
            super::panel::continue_read(session, previous, cursor)
        }
        PageStep::Read { method, params } => super::panel::request(session, method, params),
        PageStep::Complete => {
            complete(session, scope, update);
            return;
        }
    };
    session
        .codex_panel
        .snapshot
        .as_mut()
        .expect("panel read")
        .request = request;
}

fn complete(session: &mut SessionState, scope: PanelScope<'_>, update: &mut PanelUpdate) {
    let read = session
        .codex_panel
        .snapshot
        .take()
        .expect("complete panel read");
    session.codex_panel.refreshed = Some(Instant::now());
    if read.revision != session.codex_panel.revision {
        session.codex_panel.notice = "Agents changed while reading; refreshing…".into();
        session.codex_panel.refreshed = None;
        if scope.visible {
            super::panel::refresh(session, scope, false);
        }
        return;
    }
    if let Err(error) = publish(session, scope, update, read) {
        session.codex_panel.error = Some(error);
    }
}

fn publish(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
    read: Snapshot,
) -> Result<(), String> {
    if read.root != *session.thread_id() {
        return Err("Task read belongs to a retired root; previous rows retained".into());
    }
    session.codex_panel.ownership = super::panel_state::VerifiedOwners::new(
        &read.root,
        scope.native.epoch,
        read.verified_owners(),
    )?;
    let notice = read.notice();
    session.codex_panel.error = if read.errors.is_empty() {
        None
    } else {
        Some(read.errors.join("\n"))
    };
    session.codex_panel.agents = read.agents;
    session.codex_panel.processes = read.processes;
    session.codex_panel.process_scopes = read.process_scopes;
    session.codex_panel.older = read.older;
    session.codex_panel.older_window = read.older_window;
    session.codex_panel.notice = notice.into();
    if scope.selected.is_none() {
        update.selected = session.codex_panel.ids().first().cloned();
    }
    super::task_status::panel_updated(session, scope);
    Ok(())
}
