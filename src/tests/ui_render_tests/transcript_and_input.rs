// Tests for transcript layout and input-area rendering.

#[test]
fn build_rendered_lines_with_hidden_omits_selected_user_message() {
    let messages = vec![
        crate::app::tests::test_message(carlos_core::session::models::EntryDraft {
            tool_output: None,
            role: Role::User,
            text: "first prompt".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        }),
        crate::app::tests::test_message(carlos_core::session::models::EntryDraft {
            tool_output: None,
            role: Role::Assistant,
            text: "reply".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        }),
    ];

    let rendered = build_rendered_lines_with_hidden(&messages, 80, Some(0));
    assert!(!rendered.iter().any(|l| l.text.contains("first prompt")));
    assert!(rendered.iter().any(|l| l.text.contains("reply")));
}

#[test]
fn append_message_coalesces_successive_read_summaries() {
    let mut app = AppState::new("thread".to_string());
    app.append_message(Role::ToolCall, "→ Read src/main.rs [offset=10]");
    app.append_message(Role::ToolCall, "→ Read src/main.rs");
    app.append_message(Role::ToolCall, "→ Read src/main.rs [offset=30]");
    app.append_message(Role::ToolCall, "→ Read src/lib.rs");

    assert_eq!(app.session.messages().len(), 4);
    assert_eq!(app.session.messages()[0].text, "→ Read src/main.rs ×3");
    assert_eq!(app.session.messages()[1].text, "");
    assert_eq!(app.session.messages()[2].text, "");
    assert_eq!(app.session.messages()[3].text, "→ Read src/lib.rs");

    let rendered = build_rendered_lines_with_hidden(app.session.messages(), 80, None);
    let visible: Vec<_> = rendered
        .iter()
        .filter(|line| !line.separator)
        .map(|line| line.text.as_str())
        .collect();
    assert!(visible.contains(&"src/main.rs ×3"));
    assert!(visible.contains(&"src/lib.rs"));
}

#[test]
fn append_message_does_not_coalesce_read_summaries_across_other_messages() {
    let mut app = AppState::new("thread".to_string());
    app.append_message(Role::ToolCall, "→ Read src/main.rs");
    app.append_message(Role::Assistant, "working");
    app.append_message(Role::ToolCall, "→ Read src/main.rs");

    assert_eq!(app.session.messages().len(), 3);
    assert_eq!(app.session.messages()[0].text, "→ Read src/main.rs");
    assert_eq!(app.session.messages()[1].text, "working");
    assert_eq!(app.session.messages()[2].text, "→ Read src/main.rs");
}

#[test]
fn set_command_override_coalesces_with_previous_read_summary() {
    let mut app = AppState::new("thread".to_string());
    app.append_message(Role::ToolCall, "→ Read src/main.rs");
    let idx = app.append_message(Role::ToolCall, "");
    app.put_item_mapping("call-2", idx);

    app.set_command_override("call-2", "→ Read src/main.rs [offset=40]".to_string());

    assert_eq!(app.session.messages()[0].text, "→ Read src/main.rs ×2");
    assert_eq!(app.session.messages()[1].text, "");
}

#[test]
fn ensure_rendered_lines_incremental_append_matches_full_rebuild() {
    let mut view = render_fixture::view(vec![
        render_fixture::row(Role::User, "prompt one"),
        render_fixture::row(Role::Assistant, "reply one"),
        render_fixture::row(Role::ToolOutput, "line one\nline two"),
    ]);
    view.ensure_rendered_lines(48);
    let before_lines = view.rendered_line_count();
    let index = render_fixture::append(
        &mut view,
        Role::Assistant,
        "tail reply with enough text to wrap across multiple transcript rows for cache testing",
    );
    assert_eq!(view.view.render_cache.transcript_dirty_from, Some(index));
    let actual = render_fixture::snapshot(&mut view, 48);
    let mut fresh = render_fixture::view(view.rows.clone());
    let expected = render_fixture::snapshot(&mut fresh, 48);
    assert_eq!(rendered_signature(&actual), rendered_signature(&expected));
    assert_eq!(view.view.render_cache.entry_count(), view.rows.len());
    assert_eq!(view.view.render_cache.transcript_dirty_from, None);
    assert!(view.rendered_line_count() > before_lines);
}

#[test]
fn ensure_rendered_lines_incremental_answer_growth_matches_full_rebuild() {
    let mut view = render_fixture::view(vec![
        render_fixture::row(Role::User, "prompt one"),
        render_fixture::row(Role::Assistant, "starting answer"),
    ]);
    view.ensure_rendered_lines(52);
    let text = format!(
        "{}{}",
        view.rows[1].text(),
        "\ncontinued answer with more text so the last message grows substantially"
    );
    render_fixture::replace(&mut view, 1, text);
    assert_eq!(view.view.render_cache.transcript_dirty_from, Some(1));
    let actual = render_fixture::snapshot(&mut view, 52);
    let mut fresh = render_fixture::view(view.rows.clone());
    let expected = render_fixture::snapshot(&mut fresh, 52);
    assert_eq!(rendered_signature(&actual), rendered_signature(&expected));
    assert_eq!(view.view.render_cache.entry_count(), view.rows.len());
    assert_eq!(view.view.render_cache.transcript_dirty_from, None);
}

#[test]
fn ensure_rendered_lines_non_user_fence_counts_match_rendered_block() {
    let text = "before\n```text\nliteral fence payload that should remain visible\n```\nafter";
    let mut view = render_fixture::view(vec![render_fixture::row(Role::ToolOutput, text)]);
    let actual = render_fixture::snapshot(&mut view, 48);
    let expected = build_rendered_lines(
        &[crate::app::tests::test_message(
            carlos_core::session::models::EntryDraft {
                role: Role::ToolOutput,
                kind: MessageKind::Plain,
                text: text.into(),
                file_path: None,
                tool_output: None,
            },
        )],
        48,
    );
    assert_eq!(view.rendered_line_count(), expected.len());
    assert_eq!(rendered_signature(&actual), rendered_signature(&expected));
}

#[test]
fn wrap_natural_by_cells_prefers_word_boundaries() {
    let parts = wrap_natural_by_cells("alpha beta gamma", 10);
    assert_eq!(parts, vec!["alpha beta".to_string(), "gamma".to_string()]);
}

#[test]
fn wrap_natural_by_cells_keeps_trailing_space_visible() {
    let parts = wrap_natural_by_cells("test ", 10);
    assert_eq!(parts, vec!["test ".to_string()]);
}

#[test]
fn wrap_natural_by_cells_expands_tabs_for_display_width() {
    let parts = wrap_natural_by_cells("a\tb", 20);
    assert_eq!(parts, vec!["a       b".to_string()]);
}

#[test]
fn wrap_natural_count_by_cells_matches_wrapped_output_len() {
    for (text, width) in [
        ("alpha beta gamma", 10),
        ("test ", 10),
        ("averyveryverylongtoken", 6),
        ("ends with many spaces   ", 5),
    ] {
        assert_eq!(
            wrap_natural_count_by_cells(text, width),
            wrap_natural_by_cells(text, width).len()
        );
    }
}

#[test]
fn wrap_input_line_uses_natural_word_wrapping() {
    let parts = wrap_input_line("alpha beta gamma", 10);
    assert_eq!(parts, vec!["alpha beta".to_string(), "gamma".to_string()]);
}

#[test]
fn wrap_input_line_keeps_trailing_space_visible() {
    let parts = wrap_input_line("test ", 10);
    assert_eq!(parts, vec!["test ".to_string()]);
}

#[test]
fn build_rendered_lines_expands_user_tabs_for_display() {
    let messages = vec![crate::app::tests::test_message(
        carlos_core::session::models::EntryDraft {
            tool_output: None,
            role: Role::User,
            text: "col1\tcol2".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
    )];

    let rendered = build_rendered_lines(&messages, 120);
    assert_eq!(rendered.len(), 1);
    assert_eq!(rendered[0].text, "col1    col2");
    assert_eq!(rendered[0].cells, 12);
}

#[test]
fn wrap_input_line_count_matches_wrapped_output_len() {
    for (text, width) in [
        ("alpha beta gamma", 10),
        ("test ", 10),
        ("averyveryverylongtoken", 6),
        ("ends with many spaces   ", 5),
    ] {
        assert_eq!(
            wrap_input_line_count(text, width),
            wrap_input_line(text, width).len()
        );
    }
}

#[test]
fn input_cursor_visual_position_tracks_wrapped_rows() {
    let (row, col) = input_cursor_visual_position("alpha beta gamma", 16, 10);
    assert_eq!(row, 1);
    assert_eq!(col, 5);
}

#[test]
fn compute_composer_layout_wraps_input_at_word_boundaries() {
    let mut input = crate::app::input::make_input_area();
    input.insert_str("alpha beta gamma");
    let layout = crate::app::render_input::compute_composer_layout(
        &input,
        TerminalSize {
            width: 1 + 1 + 10,
            height: 8,
        },
        1,
    );
    assert_eq!(layout.visible_lines, vec!["alpha beta", "gamma"]);
    assert_eq!(layout.input_height, 2);
    assert_eq!(layout.cursor_x, 1 + 5);
}

#[test]
fn input_history_up_down_cycles_and_restores_draft() {
    let mut app = AppState::new("thread-1".to_string());
    app.push_input_history("first");
    app.push_input_history("second");
    app.set_input_text("draft text");

    assert!(app.navigate_input_history_up());
    assert_eq!(app.input_text(), "second");

    assert!(app.navigate_input_history_up());
    assert_eq!(app.input_text(), "first");

    assert!(app.navigate_input_history_down());
    assert_eq!(app.input_text(), "second");

    assert!(app.navigate_input_history_down());
    assert_eq!(app.input_text(), "draft text");
}

#[test]
fn input_history_up_noops_when_empty() {
    let mut app = AppState::new("thread-1".to_string());
    app.set_input_text("draft");
    assert!(!app.navigate_input_history_up());
    assert_eq!(app.input_text(), "draft");
}

#[test]
fn esc_chord_triggers_on_second_press_within_window() {
    let mut app = AppState::new("thread-1".to_string());
    let now = std::time::Instant::now();
    assert!(!app.register_escape_press(now));
    assert!(app.register_escape_press(now + std::time::Duration::from_millis(100)));
}

#[test]
fn esc_chord_expires_after_window() {
    let mut app = AppState::new("thread-1".to_string());
    let now = std::time::Instant::now();
    assert!(!app.register_escape_press(now));
    app.expire_esc_chord(now + std::time::Duration::from_millis(800));
    assert!(!app.register_escape_press(now + std::time::Duration::from_millis(900)));
}

#[test]
fn rewind_mode_populates_latest_history_and_restores_draft() {
    let mut app = AppState::new("thread-1".to_string());
    app.push_input_history("first");
    app.push_input_history("second");
    app.set_input_text("draft");

    app.enter_rewind_mode();
    assert!(app.rewind_mode());
    assert_eq!(app.input_text(), "second");

    let _ = app.navigate_input_history_up();
    assert_eq!(app.input_text(), "first");

    app.exit_rewind_mode_restore();
    assert!(!app.rewind_mode());
    assert_eq!(app.input_text(), "draft");
}
