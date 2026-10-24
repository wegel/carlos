// Tests for Ralph marker recognition and continuation reminders.

#[test]
fn detect_turn_markers_matches_trimmed_commentary_marker_lines() {
    let messages = vec![
        Message {
            tool_output: None,
            role: Role::User,
            text: "@@BLOCKED@@".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            tool_output: None,
            role: Role::Commentary,
            text: "working...\n  @@BLOCKED@@ \nnext".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            tool_output: None,
            role: Role::Assistant,
            text: "@@COMPLETE@@".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            tool_output: None,
            role: Role::Assistant,
            text: "@@AWAITING_HUMAN@@".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
    ];

    let markers = super::ralph::detect_turn_markers(
        &messages,
        0,
        "@@COMPLETE@@",
        "@@BLOCKED@@",
        "@@AWAITING_HUMAN@@",
    );
    assert!(markers.blocked);
    assert!(markers.completed);
    assert!(markers.waiting);
}

#[test]
fn detect_turn_markers_matches_leading_marker_with_summary_tokens() {
    let messages = vec![Message {
        tool_output: None,
        role: Role::Assistant,
        text: "@@COMPLETE@@ @@COMPLETE_SUMMARY_START@@ summary".to_string(),
        kind: MessageKind::Plain,
        file_path: None,
    }];

    let markers = super::ralph::detect_turn_markers(
        &messages,
        0,
        "@@COMPLETE@@",
        "@@BLOCKED@@",
        "@@AWAITING_HUMAN@@",
    );
    assert!(markers.completed);
    assert!(!markers.blocked);
    assert!(!markers.waiting);
}

#[test]
fn detect_turn_markers_ignore_explanatory_marker_mentions() {
    let messages = vec![Message {
        tool_output: None,
        role: Role::Assistant,
        text: concat!(
            "Do not output @@COMPLETE@@ until every plan is done.\n",
            "Use @@BLOCKED@@ only for hard blockers.\n",
            "The human gate marker is @@AWAITING_HUMAN@@."
        )
        .to_string(),
        kind: MessageKind::Plain,
        file_path: None,
    }];

    let markers = super::ralph::detect_turn_markers(
        &messages,
        0,
        "@@COMPLETE@@",
        "@@BLOCKED@@",
        "@@AWAITING_HUMAN@@",
    );
    assert!(!markers.completed);
    assert!(!markers.blocked);
    assert!(!markers.waiting);
}

#[test]
fn default_continuation_prompt_repeats_stop_rules() {
    let prompt = super::ralph::DEFAULT_CONTINUATION_PROMPT;

    assert!(prompt.contains("previous turn ended without a stop marker"));
    assert!(prompt.contains("@@AWAITING_HUMAN@@"));
    assert!(prompt.contains("@@BLOCKED@@"));
    assert!(prompt.contains("@@COMPLETE@@"));
    assert!(prompt.contains("Never stop without a marker"));
    assert!(!prompt.contains("you were interrupted"));
}

