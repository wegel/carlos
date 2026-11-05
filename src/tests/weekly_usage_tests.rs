//! Check quota selection, sparse replies, pace, and asynchronous refresh deadlines.

use std::cell::Cell;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::{RateLimits, DISPLAY_INTERVAL, REFRESH_INTERVAL, REQUEST_TIMEOUT};
use crate::app::notifications::handle_server_message_line;
use crate::app::state::AppState;
use crate::app::weekly_usage::{RateWindow, WEEK_MINUTES};
use crate::backend::{BackendClient, BackendKind};
use crate::theme::{COLOR_DIFF_REMOVE, COLOR_GUTTER_USER, COLOR_TOOL_EDIT};

const NOW: i64 = 1_800_000_000;

fn window(used: u64, days: f64) -> Value {
    json!({"usedPercent":used,"windowDurationMins":WEEK_MINUTES,
        "resetsAt":NOW+(days*86_400.0) as i64})
}

#[test]
fn weekly_pace_compares_allowance_left_with_time_left() {
    for (used, days, hue) in [
        (60, 2.1, COLOR_GUTTER_USER),
        (60, 3.5, COLOR_TOOL_EDIT),
        (60, 6.0, COLOR_DIFF_REMOVE),
        (100, 0.01, COLOR_DIFF_REMOVE),
        (0, 7.0, COLOR_GUTTER_USER),
    ] {
        let quota = RateWindow::merge(None, &window(used, days)).expect("weekly window");
        let label = quota.indicator(NOW).expect("current weekly quota");
        assert_eq!(label.percent, format!("{}%", 100 - used));
        assert_eq!(label.countdown, format!("{days:.1}d"));
        assert_eq!(label.color, hue);
    }
}

#[test]
fn quota_never_claims_an_automatic_reset_or_guesses_a_missing_window() {
    for value in [
        json!({"usedPercent":60,"resetsAt":NOW+1000}),
        json!({"usedPercent":60,"windowDurationMins":300,"resetsAt":NOW+1000}),
        json!({"usedPercent":60,"windowDurationMins":WEEK_MINUTES}),
        window(60, 0.0),
        window(60, -1.0),
    ] {
        assert!(RateWindow::merge(None, &value)
            .expect("window")
            .indicator(NOW)
            .is_none());
    }
    assert!(RateWindow::merge(None, &json!({"usedPercent":-1})).is_none());
    assert!(RateWindow::merge(None, &json!({"usedPercent":"60"})).is_none());
    assert_eq!(
        RateWindow::merge(None, &window(101, 2.1))
            .expect("clamped")
            .used,
        100
    );
}

#[test]
fn sparse_notifications_keep_the_week_and_ignore_other_buckets() {
    let mut app = AppState::new("thread".to_string());
    for snapshot in [
        json!({"limitId":"codex","secondary":window(60,2.1)}),
        json!({"limitId":"codex","primary":{"usedPercent":12,"windowDurationMins":300},
            "secondary":null}),
        json!({"limitId":"codex_other","secondary":window(99,6.0)}),
        json!({"limitId":null,"secondary":{"usedPercent":65,"resetsAt":null}}),
    ] {
        handle_server_message_line(
            &mut app,
            &json!({"method":"account/rateLimits/updated",
            "params":{"rateLimits":snapshot}})
            .to_string(),
        );
    }
    assert_eq!(
        app.rate_limits
            .indicator_at(NOW)
            .expect("weekly quota")
            .percent,
        "35%"
    );
    app.rate_limits
        .update(&json!({"primary":window(20,3.0),"secondary":{
        "usedPercent":4,"windowDurationMins":300}}));
    assert_eq!(
        app.rate_limits
            .indicator_at(NOW)
            .expect("weekly primary")
            .percent,
        "80%"
    );
}

#[test]
fn read_selects_the_shared_bucket_and_keeps_newer_notifications() {
    let mut limits = RateLimits::default();
    limits.pending = Some(("read-1".to_string(), Instant::now() + REQUEST_TIMEOUT));
    limits.update(&json!({"limitId":"codex","primary":{"usedPercent":70}}));
    assert!(!limits.response(&json!({"id":"stale-read","result":{}})));
    assert!(limits.response(&json!({"id":"read-1","result":{
        "rateLimits":{"limitId":"codex_other","primary":window(99,6.0)},
        "rateLimitsByLimitId":{"codex":{"primary":window(60,2.1)}}}})));
    let label = limits.indicator_at(NOW).expect("fresh merged weekly quota");
    assert_eq!(label.percent, "30%");
    assert_eq!(label.countdown, "2.1d");
}

#[test]
fn account_changes_discard_old_quota_and_in_flight_reads() {
    let mut app = AppState::new("thread".to_string());
    app.rate_limits.update(&json!({"secondary":window(60,2.1)}));
    app.rate_limits.pending = Some(("old-account".to_string(), Instant::now() + REQUEST_TIMEOUT));
    handle_server_message_line(
        &mut app,
        &json!({"method":"account/updated",
        "params":{"authMode":null}})
        .to_string(),
    );
    assert!(app.rate_limits.indicator_at(NOW).is_none());
    assert!(!app.rate_limits.response(&json!({"id":"old-account",
        "result":{"rateLimits":{"secondary":window(0,7.0)}}})));
}

struct QuotaClient {
    reads: Cell<u32>,
}

impl BackendClient for QuotaClient {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn call(&self, _: &str, _: Value, _: Duration) -> Result<String> {
        panic!("quota reads must not wait on synchronous calls")
    }
    fn request_rate_limits(&self) -> Result<Option<String>> {
        self.reads.set(self.reads.get() + 1);
        Ok(Some(format!("read-{}", self.reads.get())))
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        bail!("unused")
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("unused")
    }
    fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>> {
        bail!("unused")
    }
    fn stop(&mut self) {}
}

#[test]
fn refreshes_are_bounded_and_idle_countdowns_keep_waking() {
    let client = QuotaClient {
        reads: Cell::new(0),
    };
    let now = Instant::now();
    let mut limits = RateLimits::default();
    assert!(limits.tick(&client, now));
    assert_eq!(client.reads.get(), 1);
    assert_eq!(limits.wait(now), Some(REQUEST_TIMEOUT));
    assert!(!limits.tick(&client, now + REQUEST_TIMEOUT));
    assert!(limits.pending.is_none());
    assert_eq!(
        limits.wait(now + REQUEST_TIMEOUT),
        Some(DISPLAY_INTERVAL - REQUEST_TIMEOUT)
    );
    assert!(limits.tick(&client, now + DISPLAY_INTERVAL));
    assert_eq!(client.reads.get(), 1);
    assert!(limits.tick(&client, now + REFRESH_INTERVAL));
    assert_eq!(client.reads.get(), 2);
    assert!(!limits.response(&json!({"id":"read-1","result":{}})));
    assert!(limits.response(&json!({"id":"read-2","error":{"code":-32601}})));
    assert!(limits.indicator_at(NOW).is_none());
}
