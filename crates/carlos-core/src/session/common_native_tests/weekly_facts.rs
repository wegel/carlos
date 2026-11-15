//! Keep native quota selection, account invalidation and sparse read replies exact.

use std::cell::Cell;
use std::time::Instant;

use serde_json::json;

use super::weekly_test_support::{window, QuotaClient, NOW};
use super::{feed as handle_server_message_line, session};
use crate::session::rate_limits::RateLimits;
use crate::session::usage::{RateWindow, WEEK_MINUTES};

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
            .used(),
        100
    );
}

#[test]
fn sparse_notifications_keep_the_week_and_ignore_other_buckets() {
    let mut app = session("thread");
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
        app.rate_limits()
            .indicator_at(NOW)
            .expect("weekly quota")
            .remaining(),
        35
    );
    app.update_rate_limits(&json!({"primary":window(20,3.0),"secondary":{
        "usedPercent":4,"windowDurationMins":300}}));
    assert_eq!(
        app.rate_limits()
            .indicator_at(NOW)
            .expect("weekly primary")
            .remaining(),
        80
    );
}

#[test]
fn read_selects_the_shared_bucket_and_keeps_newer_notifications() {
    let mut limits = RateLimits::default();
    limits.tick(
        &QuotaClient {
            reads: Cell::new(0),
        },
        Instant::now(),
    );
    limits.update(&json!({"limitId":"codex","primary":{"usedPercent":70}}));
    assert!(!limits.response(&json!({"id":"stale-read","result":{}})));
    assert!(limits.response(&json!({"id":"read-1","result":{
        "rateLimits":{"limitId":"codex_other","primary":window(99,6.0)},
        "rateLimitsByLimitId":{"codex":{"primary":window(60,2.1)}}}})));
    let label = limits.indicator_at(NOW).expect("fresh merged weekly quota");
    assert_eq!(label.remaining(), 30);
    assert_eq!(label.remaining_seconds(), (2.1 * 86_400.0) as i64);
}

#[test]
fn account_changes_discard_old_quota_and_in_flight_reads() {
    let mut app = session("thread");
    app.update_rate_limits(&json!({"secondary":window(60,2.1)}));
    app.tick_rate_limits(
        &QuotaClient {
            reads: Cell::new(0),
        },
        Instant::now(),
    );
    handle_server_message_line(
        &mut app,
        &json!({"method":"account/updated",
        "params":{"authMode":null}})
        .to_string(),
    );
    assert!(app.rate_limits().indicator_at(NOW).is_none());
    assert!(!app.rate_limits_response(&json!({"id":"read-1",
        "result":{"rateLimits":{"secondary":window(0,7.0)}}})));
}
