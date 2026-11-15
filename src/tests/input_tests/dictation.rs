// Tests for dictation key shortcuts and state transitions.

#[test]
fn prioritize_events_handles_terminal_first_and_budgets_server_lines() {
    let mut deferred = std::collections::VecDeque::new();
    let incoming = vec![
        UiEvent::ServerLine("s1".to_string()),
        UiEvent::Terminal(Event::Resize(80, 24)),
        UiEvent::ServerLine("s2".to_string()),
    ];

    let prioritized = prioritize_events(incoming, &mut deferred, 1);
    assert_eq!(prioritized.len(), 2);
    assert!(matches!(
        prioritized[0],
        UiEvent::Terminal(Event::Resize(80, 24))
    ));
    assert!(matches!(prioritized[1], UiEvent::ServerLine(ref s) if s == "s1"));
    assert_eq!(deferred.len(), 1);
    assert_eq!(deferred.front().map(String::as_str), Some("s2"));
}
