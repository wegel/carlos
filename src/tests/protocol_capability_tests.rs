//! Probe supported CLI feature names without starting an agent or waiting indefinitely.

use std::process::Command;
use std::time::{Duration, Instant};

use super::probe;

fn python(script: &str) -> Command {
    let mut command = Command::new("python3");
    command.args(["-c", script]);
    command
}

#[test]
fn only_a_precise_supported_live_settings_feature_allows_the_process_flag() {
    for (output, expected) in [
        ("step_model_switching under development false", true),
        ("step_model_switching stable true", true),
        ("step_model_switching removed false", false),
        ("other_step_model_switching stable true", false),
        ("Unknown method", false),
    ] {
        let script = format!("print({output:?})");
        assert_eq!(
            probe(python(&script), Duration::from_secs(1)).live_settings,
            expected
        );
    }
    assert!(!probe(python("raise SystemExit(2)"), Duration::from_secs(1)).live_settings);
}

#[test]
fn feature_probe_bounds_slow_and_oversized_output_without_hanging_startup() {
    let started = Instant::now();
    assert!(
        !probe(
            python("import time;time.sleep(60)"),
            Duration::from_millis(50)
        )
        .live_settings
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(!probe(python("print('x'*100000)"), Duration::from_secs(1)).live_settings);
}

#[test]
fn goals_flag_is_added_only_for_an_advertised_default_off_feature() {
    for (output, expected) in [
        ("goals stable false", true),
        ("goals stable true", false),
        ("goals deprecated false", false),
        ("other_goals stable false", false),
    ] {
        assert_eq!(
            probe(
                python(&format!("print({output:?})")),
                Duration::from_secs(1)
            )
            .enable_goals,
            expected
        );
    }
}
