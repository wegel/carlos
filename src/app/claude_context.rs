//! Refresh root context snapshots without clearing known counts during ordinary work.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::claude_state::ClaudePhase;
use super::context_usage::ContextUsage;
use super::state::AppState;
use crate::backend::BackendClient;

const INTERVAL: Duration = Duration::from_secs(1);
const STALE_AFTER: Duration = Duration::from_secs(5);

/// Keep one native read outstanding, including when a slow reply outlives its UI deadline.
#[derive(Default)]
pub(super) struct ClaudeContext {
    pending: Option<Read>,
    next_read: Option<Instant>,
    sequence: u64,
    pub(super) fresh: bool,
    model: Option<String>,
}

struct Read {
    token: u64,
    sent: Instant,
}

/// Request another snapshot without invalidating the last known native count.
pub(super) fn refresh(app: &mut AppState) {
    app.claude.context.fresh = false;
}

/// Fence old reads only when the native context or its window actually changes.
pub(super) fn invalidate(app: &mut AppState) {
    app.claude.activity.context_revision += 1;
    app.context_usage = None;
    app.claude.context.next_read = None;
    refresh(app);
}

/// Send at most one optional summary read; neither typing nor chat waits for it.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState, now: Instant) -> bool {
    if !available(app) {
        return false;
    }
    let model = app.runtime.current_model.clone();
    let mut redraw = false;
    if app.claude.context.model != model {
        redraw = app.context_usage.is_some();
        app.claude.context.model = model;
        invalidate(app);
    }
    if app.claude.context.fresh && !working(app) && app.claude.context.pending.is_none() {
        return redraw;
    }
    let context = &mut app.claude.context;
    if let Some(read) = &context.pending {
        let stale = now.saturating_duration_since(read.sent) >= STALE_AFTER;
        let changed = stale && context.fresh;
        context.fresh &= !stale;
        return redraw || changed;
    }
    if context.next_read.is_some_and(|deadline| deadline > now) {
        return redraw;
    }
    context.sequence += 1;
    context.pending = Some(Read {
        token: context.sequence,
        sent: now,
    });
    context.next_read = Some(now + INTERVAL);
    let params =
        json!({"revision":app.claude.activity.context_revision,"contextToken":context.sequence});
    if client.request_native_control("context", params).is_err() {
        context.pending = None;
        context.next_read = Some(now + STALE_AFTER);
        redraw |= context.fresh;
        context.fresh = false;
    }
    redraw
}

/// Wake idle work for reads and stale-value markers, without polling a stuck request forever.
pub(super) fn wait(app: &AppState, now: Instant) -> Option<Duration> {
    if !available(app) {
        return None;
    }
    let context = &app.claude.context;
    if context.fresh && !working(app) && context.pending.is_none() {
        return None;
    }
    let deadline = if let Some(read) = &context.pending {
        let deadline = read.sent + STALE_AFTER;
        (deadline > now).then_some(deadline)?
    } else {
        context.next_read.unwrap_or(now)
    };
    Some(deadline.saturating_duration_since(now))
}

/// Settle even an obsolete read, then apply only counts for the current context revision.
pub(super) fn receive(app: &mut AppState, frame: &Value, request: &Value) {
    let context = &mut app.claude.context;
    if let Some(token) = request["contextToken"].as_u64() {
        if !context
            .pending
            .as_ref()
            .is_some_and(|read| read.token == token)
        {
            return;
        }
    }
    context.pending = None;
    if request["revision"]
        .as_u64()
        .is_some_and(|revision| revision != app.claude.activity.context_revision)
    {
        context.next_read = None;
        return;
    }
    context.next_read = Some(
        Instant::now()
            + if frame["ok"] == true {
                INTERVAL
            } else {
                STALE_AFTER
            },
    );
    let result = &frame["result"];
    let max = result["rawMaxTokens"]
        .as_u64()
        .or_else(|| result["maxTokens"].as_u64());
    if frame["ok"] == true {
        if let (Some(used), Some(max)) =
            (result["totalTokens"].as_u64(), max.filter(|max| *max > 0))
        {
            app.context_usage = Some(ContextUsage { used, max });
            context.fresh = true;
            return;
        }
    }
    context.fresh = false;
}

fn available(app: &AppState) -> bool {
    app.claude.enabled
        && app.claude.phase == ClaudePhase::Ready
        && app.claude.activity.transient.as_deref() != Some("Compacting")
}

fn working(app: &AppState) -> bool {
    app.active_turn_id.is_some()
        || app.claude.activity.state != super::claude_activity::NativeState::Idle
}
