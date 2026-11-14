//! Schedule and expire independently addressed child reads within one bounded native request queue.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::panel::request;
use super::panel_scope::{PanelScope, PanelUpdate};
use super::panel_state::{Activity, PanelId, Request};
use crate::session::state::SessionState;

/// Inspect a selected child without resuming it or changing its model or history.
pub fn details(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
    older: bool,
) {
    let Some(PanelId::Agent(thread)) = scope.selected.cloned() else {
        return;
    };
    if !session.codex_panel.contains_target(scope.selected) {
        return;
    }
    if !older || !session.codex_panel.activities.contains_key(&thread) {
        if !session.codex_panel.activities.contains_key(&thread)
            && session.codex_panel.activities.len() >= 8
        {
            session.codex_panel.error =
                Some("At most eight child detail interests can be active".into());
            return;
        }
        session.codex_panel.activities.insert(
            thread.clone(),
            Activity {
                thread: thread.clone(),
                request: None,
                started: Instant::now(),
                items: Vec::new(),
                cursor: Value::Null,
                cursors: HashSet::new(),
                bytes: 0,
                tail: String::new(),
                error: None,
                more: true,
                until: 20,
            },
        );
    }
    let activity = session
        .codex_panel
        .activities
        .get_mut(&thread)
        .expect("addressed activity");
    if activity.request.is_some() || !activity.more || activity.items.len() >= 128 {
        return;
    }
    activity.started = Instant::now();
    activity.until = (activity.items.len() + 20).min(128);
    let params =
        json!({"threadId":thread,"sortDirection":"desc","limit":1,"cursor":activity.cursor});
    let request = request(session, "thread/items/list", params);
    session
        .codex_panel
        .activities
        .get_mut(&thread)
        .expect("addressed activity")
        .request = Some(request);
    update.detail = true;
    update.reset_scroll = true;
}

pub(super) fn timeout(session: &SessionState, request: &Request) -> u64 {
    let started = session
        .codex_panel
        .snapshot
        .as_ref()
        .filter(|read| read.request.token == request.token)
        .map(|read| read.started)
        .or_else(|| {
            session
                .codex_panel
                .activities
                .values()
                .find(|activity| {
                    activity
                        .request
                        .as_ref()
                        .is_some_and(|pending| pending.token == request.token)
                })
                .map(|activity| activity.started)
        })
        .or_else(|| {
            session
                .codex_panel
                .runs
                .values()
                .find(|read| {
                    read.request
                        .as_ref()
                        .is_some_and(|pending| pending.token == request.token)
                })
                .map(|read| read.started)
        });
    if let Some(started) = started {
        return Duration::from_secs(20)
            .saturating_sub(started.elapsed())
            .as_millis()
            .clamp(1, 5000) as u64;
    }
    if crate::protocol::is_read_only(&request.method) {
        5000
    } else {
        15000
    }
}

pub(super) fn expire(session: &mut SessionState) -> bool {
    let mut expired = Vec::new();
    if let Some(read) = session
        .codex_panel
        .snapshot
        .as_ref()
        .filter(|read| read.started.elapsed() >= Duration::from_secs(20))
    {
        expired.push(read.request.token);
        session.codex_panel.snapshot = None;
        session.codex_panel.error =
            Some("Panel refresh exceeded20 seconds; previous rows retained".into());
        session.codex_panel.refreshed = Some(Instant::now());
    }
    for activity in session
        .codex_panel
        .activities
        .values_mut()
        .filter(|activity| {
            activity.request.is_some() && activity.started.elapsed() >= Duration::from_secs(20)
        })
    {
        expired.push(activity.request.take().expect("expired child read").token);
        activity.error =
            Some("Child read exceeded20 seconds; retained items remain available".into());
    }
    for read in
        session.codex_panel.runs.values_mut().filter(|read| {
            read.request.is_some() && read.started.elapsed() >= Duration::from_secs(20)
        })
    {
        expired.push(read.request.take().expect("expired native run read").token);
        read.turn = None;
        read.error = Some("Task run read exceeded 20 seconds".into());
    }
    session
        .codex_panel
        .outgoing
        .retain(|request| !expired.contains(&request.token));
    !expired.is_empty()
}
