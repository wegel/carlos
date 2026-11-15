//! Schedule bounded Codex panel reads and exact selected controls without blocking the UI.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

pub use super::panel_reads::details;
use super::panel_reads::{expire, timeout};
use super::panel_scope::{PanelScope, PanelUpdate};
use super::panel_state::{PanelId, Request, Snapshot, Stop};
use crate::backend::BackendClient;
use crate::session::state::SessionState;

/// Poll only while the panel is open; pending controls still settle after it closes.
pub fn tick(
    client: &dyn BackendClient,
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
) -> bool {
    if !scope.native.enabled || scope.native.closed {
        return false;
    }
    if scope.visible
        && !session.codex_panel.older_window
        && !scope.detail
        && session.codex_panel.snapshot.is_none()
        && session
            .codex_panel
            .refreshed
            .is_none_or(|at| at.elapsed() >= Duration::from_secs(3))
    {
        refresh(session, scope, false);
    }
    session.expire_task_windows();
    let mut changed = expire(session);
    while let Some(request) = session.codex_panel.outgoing.pop_front() {
        let context = json!({"operation":"panel","epoch":scope.native.epoch,
            "root":session.thread_id(),"token":request.token,"timeoutMs":timeout(session, &request)});
        if let Err(error) =
            client.request_control_with_context(&request.method, request.params.clone(), context)
        {
            super::panel_receive::failed(session, scope, update, &request, &error.to_string());
        }
        changed = true;
    }
    changed
}

/// Start a recent or explicitly older bounded agent window without replacing a usable snapshot yet.
pub fn refresh(session: &mut SessionState, scope: PanelScope<'_>, older: bool) {
    if scope.native.closed || session.codex_panel.snapshot.is_some() {
        return;
    }
    let cursor = if older {
        session.codex_panel.older.clone()
    } else {
        Value::Null
    };
    if older && cursor.is_null() {
        return;
    }
    let mut read = Snapshot::new(
        session.thread_id(),
        session.codex_panel.revision,
        cursor,
        older,
    );
    read.request = request(session, &read.request.method, read.request.params.clone());
    session.codex_panel.snapshot = Some(read);
    session.codex_panel.notice = "Reading agents and background processes…".into();
}

/// Stop only a currently selected verified native identity; never redirect a vanished selection.
pub fn stop(session: &mut SessionState, scope: PanelScope<'_>) {
    if scope.native.closed || session.codex_panel.stop.is_some() {
        return;
    }
    if !session.codex_panel.contains_target(scope.selected) {
        session.codex_panel.error =
            Some("Selected row left the list; use arrows to choose another".into());
        return;
    }
    let target = scope.selected.cloned().expect("selected native row");
    let (method, params) = match &target {
        PanelId::Agent(thread) => (
            "thread/turns/list",
            json!({"threadId":thread,
            "sortDirection":"desc","itemsView":"notLoaded","limit":1}),
        ),
        PanelId::Process {
            thread, process, ..
        } => (
            "thread/backgroundTerminals/terminate",
            json!({"threadId":thread,"processId":process}),
        ),
    };
    let request = request(session, method, params);
    session.codex_panel.stop = Some(Stop {
        target,
        request,
        message: "Checking the selected native target…".into(),
    });
    session.codex_panel.control_error = None;
}

/// Queue one bounded operation with a caller-owned token that never reaches native parameters.
pub fn request(session: &mut SessionState, method: &str, params: Value) -> Request {
    let mut tokens: Vec<_> = session
        .codex_panel
        .activities
        .values()
        .filter_map(|activity| activity.request.as_ref().map(|request| request.token))
        .collect();
    tokens.extend(session.task_windows.slots.values().filter_map(|slot| {
        if let crate::session::task_window_state::Contents::Reading(read) = &slot.contents {
            Some(read.request.token)
        } else {
            None
        }
    }));
    tokens.extend(
        session
            .codex_panel
            .snapshot
            .as_ref()
            .map(|read| read.request.token),
    );
    tokens.extend(
        session
            .codex_panel
            .runs
            .values()
            .filter_map(|read| read.request.as_ref().map(|request| request.token)),
    );
    tokens.extend(
        session
            .codex_panel
            .stop
            .as_ref()
            .map(|stop| stop.request.token),
    );
    session
        .codex_panel
        .outgoing
        .retain(|request| tokens.contains(&request.token));
    session.advance_codex_serial();
    let request = Request {
        token: session.codex_serial(),
        method: method.into(),
        params,
    };
    session.codex_panel.outgoing.push_back(request.clone());
    request
}

/// Continue a page with the same read token and original deadline.
pub(crate) fn continue_read(
    session: &mut SessionState,
    mut request: Request,
    cursor: Value,
) -> Request {
    request.params["cursor"] = cursor;
    session.codex_panel.outgoing.push_back(request.clone());
    request
}

/// Wake an idle UI for periodic F2 reads and original read deadlines without polling hidden views.
pub fn wait(session: &SessionState, scope: PanelScope<'_>, now: Instant) -> Option<Duration> {
    if !scope.native.enabled || scope.native.closed {
        return None;
    }
    let panel = &session.codex_panel;
    let snapshot = panel
        .snapshot
        .as_ref()
        .map(|read| (read.started + Duration::from_secs(20)).saturating_duration_since(now));
    let activity = panel
        .activities
        .values()
        .filter(|activity| activity.request.is_some())
        .map(|activity| {
            (activity.started + Duration::from_secs(20)).saturating_duration_since(now)
        });
    let runs = panel
        .runs
        .values()
        .filter(|read| read.request.is_some())
        .map(|read| (read.started + Duration::from_secs(20)).saturating_duration_since(now));
    let refresh =
        (scope.visible && !panel.older_window && !scope.detail && panel.snapshot.is_none()).then(
            || {
                panel.refreshed.map_or(Duration::ZERO, |at| {
                    (at + Duration::from_secs(3)).saturating_duration_since(now)
                })
            },
        );
    snapshot
        .into_iter()
        .chain(activity)
        .chain(runs)
        .chain(refresh)
        .chain(session.task_windows.wait(now))
        .min()
}
