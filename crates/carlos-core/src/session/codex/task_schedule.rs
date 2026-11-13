//! Schedule bounded reads for verified native process owners.

use std::collections::{BTreeSet, HashSet};
use std::time::{Duration, Instant};

use serde_json::json;

use super::Read;
use crate::session::codex::panel_scope::PanelScope;
use crate::session::codex::panel_state::Request;
use crate::session::codex::task_badge;
use crate::session::state::SessionState;

/// Choose a verified owner whose bounded native read is due.
pub(super) fn schedule(session: &mut SessionState, scope: PanelScope<'_>, now: Instant) {
    if session.codex_task_status.read.is_none() && session.codex_panel.snapshot.is_none() {
        if session.codex_task_status.next.is_some_and(|at| now >= at) {
            let owners = owners(session, scope);
            session.codex_task_status.dirty.extend(owners);
            session.codex_task_status.next = None;
        }
        let next = session
            .codex_task_status
            .dirty
            .iter()
            .find(|owner| {
                task_badge::owns(session, scope, owner)
                    && session
                        .codex_task_status
                        .allowed
                        .get(*owner)
                        .is_none_or(|at| now >= *at)
            })
            .cloned();
        if let Some(owner) = next {
            begin(session, scope, &owner, now);
        }
    }
}

/// Create one targeted read under its original token and generation.
pub(super) fn begin(session: &mut SessionState, scope: PanelScope<'_>, owner: &str, now: Instant) {
    session.codex_task_status.dirty.remove(owner);
    session
        .codex_task_status
        .allowed
        .insert(owner.into(), now + Duration::from_secs(3));
    session.advance_codex_serial();
    session.codex_task_status.read = Some(Read {
        request: Request {
            token: session.codex_serial(),
            method: "thread/backgroundTerminals/list".into(),
            params: json!({"threadId":owner,"limit":1,"cursor":null}),
        },
        started: now,
        bytes: 0,
        items: Vec::new(),
        cursors: HashSet::new(),
        revision: *session.codex_task_status.revisions.get(owner).unwrap_or(&0),
        generation: scope.generation,
        sent: false,
    });
}

/// Mark only this native owner uncertain after an observed change.
pub(super) fn invalidate(session: &mut SessionState, owner: &str, dirty: bool) {
    *session
        .codex_task_status
        .revisions
        .entry(owner.into())
        .or_default() += 1;
    session.codex_panel.revision = session.codex_panel.revision.saturating_add(1);
    session.codex_task_status.uncertain.insert(owner.into());
    if dirty {
        session.codex_task_status.dirty.insert(owner.into());
    }
}

/// Retain the failed owner without inventing a healthy process cache.
pub(super) fn failed(session: &mut SessionState, scope: PanelScope<'_>, error: &str) {
    if let Some(read) = session.codex_task_status.read.take() {
        if let Some(owner) = read.request.params["threadId"].as_str() {
            session.codex_task_status.uncertain.insert(owner.into());
            session
                .codex_task_status
                .errors
                .insert(owner.into(), error.into());
        }
    }
    if owners(session, scope).is_empty() {
        session.codex_task_status.next = None;
    }
}

/// Collect verified owners whose process cache is still authoritative.
pub(super) fn owners(session: &SessionState, scope: PanelScope<'_>) -> BTreeSet<String> {
    session
        .codex_panel
        .processes
        .iter()
        .filter(|row| {
            task_badge::owns(session, scope, &row.thread)
                && !session.codex_task_status.uncertain.contains(&row.thread)
        })
        .map(|row| row.thread.clone())
        .collect()
}

/// Allow reads only on an open native connection outside a rebuild.
pub(super) fn connected(scope: PanelScope<'_>) -> bool {
    scope.native.enabled && !scope.native.closed && !scope.rebuilding
}

/// Drop targeted work that no longer belongs to the current root.
pub(super) fn prune(session: &mut SessionState, scope: PanelScope<'_>) {
    let mut valid: HashSet<String> = session
        .codex_panel
        .ownership
        .ids()
        .into_iter()
        .filter(|owner| task_badge::owns(session, scope, owner))
        .collect();
    valid.insert(session.thread_id().clone());
    let status = &mut session.codex_task_status;
    status.dirty.retain(|owner| valid.contains(owner));
    status.uncertain.retain(|owner| valid.contains(owner));
    status.errors.retain(|owner, _| valid.contains(owner));
    status.revisions.retain(|owner, _| valid.contains(owner));
    status.allowed.retain(|owner, _| valid.contains(owner));
    if owners(session, scope).is_empty()
        && session.codex_task_status.read.is_none()
        && session.codex_task_status.dirty.is_empty()
    {
        session.codex_task_status.next = None;
    }
}
