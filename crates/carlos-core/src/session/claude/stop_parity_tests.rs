//! Enumerate exact-run stop guards, retries, bounds, and native termination.

use super::task_stops::{StopState, TaskStops};

#[test]
fn stops_match_optional_runs_and_retry_only_failed_requests() {
    for run in [None, Some("a")] {
        let mut stops = TaskStops::default();
        assert_eq!(stops.begin("task", run), Ok(true));
        assert_eq!(stops.begin("task", run), Ok(false));
        assert!(matches!(stops.state("task", run), Some(StopState::Sending)));
        stops.reply("task", Some("other"), None);
        stops.finish("task", Some("other"));
        assert!(matches!(stops.state("task", run), Some(StopState::Sending)));
        stops.reply("task", run, Some("é".repeat(1025)));
        match stops.state("task", run) {
            Some(StopState::Failed(error)) => assert_eq!(error, &"é".repeat(1024)),
            _ => panic!("the exact request must retain its bounded failure"),
        }
        assert_eq!(stops.begin("task", run), Ok(true));
        stops.reply("task", run, None);
        assert_eq!(stops.begin("task", run), Ok(false));
        assert!(matches!(
            stops.state("task", run),
            Some(StopState::Requested)
        ));
        stops.finish("task", run);
        assert!(stops.state("task", run).is_none());
        assert_eq!(stops.begin("task", run), Ok(true));
    }
}

#[test]
fn stop_bound_allows_replacing_a_run_and_reuses_only_a_finished_slot() {
    let mut stops = TaskStops::default();
    for index in 0..4096 {
        assert_eq!(stops.begin(&format!("task-{index}"), Some("a")), Ok(true));
    }
    assert_eq!(
        stops.begin("overflow", None),
        Err("Too many unanswered native task stops; wait for native task events")
    );
    assert_eq!(stops.begin("task-0", Some("b")), Ok(true));
    assert!(stops.state("task-0", Some("a")).is_none());
    stops.finish("task-0", Some("a"));
    assert!(stops.begin("overflow", None).is_err());
    stops.finish("task-0", Some("b"));
    assert_eq!(stops.begin("overflow", None), Ok(true));
}
