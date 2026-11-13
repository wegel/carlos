//! Native stream fractions and optional report percentages retain separate quota buckets.

use serde_json::json;

use super::weekly_usage::WeeklyIndicator;
use crate::session::runtime_settings::RuntimeDefaults;
use crate::session::state::SessionState;
use crate::session::usage::unix_now;

#[test]
fn stream_fraction_is_percent_used_and_only_global_weekly_drives_the_weekly_badge() {
    let mut quota = SessionState::new("root".into(), RuntimeDefaults::default(), false);
    let reset = unix_now() + 86400;
    quota.claude_quota_event(
        &json!({"rateLimitType":"seven_day_opus","utilization":0.9,"resetsAt":reset}),
    );
    assert!(quota.claude().quota().indicator().is_none());
    quota.claude_quota_event(&json!({"rateLimitType":"five_hour","unifiedWindows":{"five_hour":{"utilization":0.25,"resetsAt":reset},"seven_day":{"utilization":0.4,"resetsAt":reset}}}));
    assert_eq!(quota.claude().quota().windows()["five_hour"].used(), 25.0);
    assert_eq!(
        WeeklyIndicator::from(quota.claude().quota().indicator().expect("global weekly")).percent,
        "60%"
    );
    assert_eq!(
        quota.claude().quota().windows()["seven_day_opus"].used(),
        90.0
    );
}

#[test]
fn sparse_late_optional_read_cannot_replace_newer_native_windows_or_mix_model_buckets() {
    let mut quota = SessionState::new("root".into(), RuntimeDefaults::default(), false);
    let reset = unix_now() + 86400;
    quota.claude_quota_event(
        &json!({"rateLimitType":"seven_day","utilization":0.4,"resetsAt":reset}),
    );
    quota.claude_quota_optional(&json!({"available":true,"windows":[{"key":"seven_day","usedPercent":80,"resetsAt":reset},{"key":"five_hour","usedPercent":20,"resetsAt":reset},{"key":"model:Fable","label":"Fable","usedPercent":7,"resetsAt":reset}]}),0);
    assert_eq!(quota.claude().quota().windows()["seven_day"].used(), 40.0);
    assert_eq!(quota.claude().quota().windows()["five_hour"].used(), 20.0);
    assert_eq!(quota.claude().quota().windows()["model:Fable"].used(), 7.0);
    quota.claude_quota_optional(&json!({"available":false,"reason":"unsupported"}), 1);
    assert_eq!(quota.claude().quota().windows().len(), 3);
    quota.claude_quota_optional(
        &json!({"available":false,"report":{"rate_limits_available":false}}),
        0,
    );
    assert!(quota.claude().quota().windows().contains_key("seven_day"));
    assert!(!quota.claude().quota().windows().contains_key("five_hour"));
}

#[test]
fn missing_expired_nonfinite_or_wrong_unit_stream_values_remain_absent() {
    let mut quota = SessionState::new("root".into(), RuntimeDefaults::default(), false);
    let reset = unix_now() + 86400;
    for value in [
        json!({"rateLimitType":"seven_day","resetsAt":reset}),
        json!({"rateLimitType":"seven_day","utilization":25,"resetsAt":reset}),
        json!({"rateLimitType":"seven_day","utilization":0.5,"resetsAt":unix_now()-1}),
    ] {
        quota.claude_quota_event(&value);
    }
    assert!(quota.claude().quota().windows().is_empty());
    assert!(quota.claude().quota().indicator().is_none());
}
