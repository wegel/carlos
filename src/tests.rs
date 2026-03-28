use super::selection::shift_selection_focus;
use super::*;

#[test]
fn compute_selection_range_normalizes_reversed_coordinates() {
    let sel = Selection {
        anchor_x: 10,
        anchor_line_idx: 5,
        focus_x: 3,
        focus_line_idx: 5,
        dragging: false,
    };
    let range = compute_selection_range(sel, 5, 20).unwrap();
    assert_eq!(range, (2, 10));
}

#[test]
fn selected_text_keeps_left_padding_in_selection() {
    let lines = vec![RenderedLine {
        text: "  hello".to_string(),
        styled_segments: Vec::new(),
        role: Role::Assistant,
        separator: false,
        cells: 7,
        soft_wrap_to_next: false,
    }];

    let sel = Selection {
        anchor_x: 1,
        anchor_line_idx: 0,
        focus_x: 4,
        focus_line_idx: 0,
        dragging: false,
    };

    let out = selected_text(sel, &lines);
    assert_eq!(out, "  he");
}

#[test]
fn selected_text_joins_soft_wrapped_rows_without_newline() {
    let lines = vec![
        RenderedLine {
            text: "abcde".to_string(),
            styled_segments: Vec::new(),
            role: Role::Assistant,
            separator: false,
            cells: 5,
            soft_wrap_to_next: true,
        },
        RenderedLine {
            text: "fghij".to_string(),
            styled_segments: Vec::new(),
            role: Role::Assistant,
            separator: false,
            cells: 5,
            soft_wrap_to_next: false,
        },
    ];

    let sel = Selection {
        anchor_x: 1,
        anchor_line_idx: 0,
        focus_x: 5,
        focus_line_idx: 1,
        dragging: false,
    };

    let out = selected_text(sel, &lines);
    assert_eq!(out, "abcdefghij");
}

#[test]
fn selected_text_keeps_newline_on_hard_break_rows() {
    let lines = vec![
        RenderedLine {
            text: "abcde".to_string(),
            styled_segments: Vec::new(),
            role: Role::Assistant,
            separator: false,
            cells: 5,
            soft_wrap_to_next: false,
        },
        RenderedLine {
            text: "fghij".to_string(),
            styled_segments: Vec::new(),
            role: Role::Assistant,
            separator: false,
            cells: 5,
            soft_wrap_to_next: false,
        },
    ];

    let sel = Selection {
        anchor_x: 1,
        anchor_line_idx: 0,
        focus_x: 5,
        focus_line_idx: 1,
        dragging: false,
    };

    let out = selected_text(sel, &lines);
    assert_eq!(out, "abcde\nfghij");
}

#[test]
fn selected_text_restores_space_for_soft_wrapped_words() {
    let lines = vec![
        RenderedLine {
            text: "Analyzing".to_string(),
            styled_segments: Vec::new(),
            role: Role::Reasoning,
            separator: false,
            cells: 9,
            soft_wrap_to_next: true,
        },
        RenderedLine {
            text: "delta stream message formatting".to_string(),
            styled_segments: Vec::new(),
            role: Role::Reasoning,
            separator: false,
            cells: 31,
            soft_wrap_to_next: false,
        },
    ];

    let sel = Selection {
        anchor_x: 1,
        anchor_line_idx: 0,
        focus_x: 31,
        focus_line_idx: 1,
        dragging: false,
    };

    let out = selected_text(sel, &lines);
    assert_eq!(out, "Analyzing delta stream message formatting");
}

#[test]
fn selected_text_soft_wrapped_commentary_strips_prefix_and_restores_space() {
    let first =
        "checking whether the remaining glitch is in the markdown renderer itself".to_string();
    let second = "or in a separate reasoning-message path.".to_string();
    let lines = vec![
        RenderedLine {
            text: first.clone(),
            styled_segments: Vec::new(),
            role: Role::Commentary,
            separator: false,
            cells: visual_width(&first),
            soft_wrap_to_next: true,
        },
        RenderedLine {
            text: second.clone(),
            styled_segments: Vec::new(),
            role: Role::Commentary,
            separator: false,
            cells: visual_width(&second),
            soft_wrap_to_next: false,
        },
    ];

    let sel = Selection {
        anchor_x: 1,
        anchor_line_idx: 0,
        focus_x: visual_width(&second),
        focus_line_idx: 1,
        dragging: false,
    };

    let out = selected_text(sel, &lines);
    assert_eq!(
        out,
        "checking whether the remaining glitch is in the markdown renderer itself or in a separate reasoning-message path."
    );
}

#[test]
fn selected_text_does_not_insert_space_into_hard_wrapped_long_token() {
    let lines = vec![
        RenderedLine {
            text: "averyveryverylong".to_string(),
            styled_segments: Vec::new(),
            role: Role::Assistant,
            separator: false,
            cells: 17,
            soft_wrap_to_next: true,
        },
        RenderedLine {
            text: "tokenwithoutspaces".to_string(),
            styled_segments: Vec::new(),
            role: Role::Assistant,
            separator: false,
            cells: 18,
            soft_wrap_to_next: false,
        },
    ];

    let sel = Selection {
        anchor_x: 1,
        anchor_line_idx: 0,
        focus_x: 18,
        focus_line_idx: 1,
        dragging: false,
    };

    let out = selected_text(sel, &lines);
    assert_eq!(out, "averyveryverylongtokenwithoutspaces");
}

fn rendered_signature(lines: &[RenderedLine]) -> Vec<(String, Role, bool, usize, bool)> {
    lines
        .iter()
        .map(|line| {
            (
                line.text.clone(),
                line.role,
                line.separator,
                line.cells,
                line.soft_wrap_to_next,
            )
        })
        .collect()
}

#[test]
fn shift_selection_focus_extends_copy_beyond_visible_screen() {
    let lines = vec![
        RenderedLine {
            text: "line 1".to_string(),
            styled_segments: Vec::new(),
            role: Role::Assistant,
            separator: false,
            cells: 6,
            soft_wrap_to_next: false,
        },
        RenderedLine {
            text: "line 2".to_string(),
            styled_segments: Vec::new(),
            role: Role::Assistant,
            separator: false,
            cells: 6,
            soft_wrap_to_next: false,
        },
        RenderedLine {
            text: "line 3".to_string(),
            styled_segments: Vec::new(),
            role: Role::Assistant,
            separator: false,
            cells: 6,
            soft_wrap_to_next: false,
        },
    ];
    let mut sel = Selection {
        anchor_x: 1,
        anchor_line_idx: 0,
        focus_x: 6,
        focus_line_idx: 1,
        dragging: true,
    };

    shift_selection_focus(&mut sel, 1, lines.len() - 1);

    let out = selected_text(sel, &lines);
    assert_eq!(out, "line 1\nline 2\nline 3");
}

#[test]
fn build_rendered_lines_inserts_separator_rows_between_messages() {
    let messages = vec![
        Message {
            role: Role::User,
            text: "first".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            role: Role::Assistant,
            text: "second".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
    ];

    let rendered = build_rendered_lines(&messages, 40);
    assert!(rendered.len() >= 3);
    assert!(rendered[1].separator);
    assert_eq!(rendered[1].role, Role::System);
}

#[test]
fn build_rendered_lines_with_hidden_omits_selected_user_message() {
    let messages = vec![
        Message {
            role: Role::User,
            text: "first prompt".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            role: Role::Assistant,
            text: "reply".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
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

    assert_eq!(app.messages.len(), 4);
    assert_eq!(app.messages[0].text, "→ Read src/main.rs ×3");
    assert_eq!(app.messages[1].text, "");
    assert_eq!(app.messages[2].text, "");
    assert_eq!(app.messages[3].text, "→ Read src/lib.rs");

    let rendered = build_rendered_lines_with_hidden(&app.messages, 80, None);
    let visible: Vec<_> = rendered
        .iter()
        .filter(|line| !line.separator)
        .map(|line| line.text.as_str())
        .collect();
    assert!(visible.contains(&"→ Read src/main.rs ×3"));
    assert!(visible.contains(&"→ Read src/lib.rs"));
}

#[test]
fn append_message_does_not_coalesce_read_summaries_across_other_messages() {
    let mut app = AppState::new("thread".to_string());
    app.append_message(Role::ToolCall, "→ Read src/main.rs");
    app.append_message(Role::Assistant, "working");
    app.append_message(Role::ToolCall, "→ Read src/main.rs");

    assert_eq!(app.messages.len(), 3);
    assert_eq!(app.messages[0].text, "→ Read src/main.rs");
    assert_eq!(app.messages[1].text, "working");
    assert_eq!(app.messages[2].text, "→ Read src/main.rs");
}

#[test]
fn set_command_override_coalesces_with_previous_read_summary() {
    let mut app = AppState::new("thread".to_string());
    app.append_message(Role::ToolCall, "→ Read src/main.rs");
    let idx = app.append_message(Role::ToolCall, "");
    app.put_agent_item_mapping("call-2", idx);

    app.set_command_override("call-2", "→ Read src/main.rs [offset=40]".to_string());

    assert_eq!(app.messages[0].text, "→ Read src/main.rs ×2");
    assert_eq!(app.messages[1].text, "");
}

#[test]
fn ensure_rendered_lines_incremental_append_matches_full_rebuild() {
    let mut app = AppState::new("thread".to_string());
    app.append_message(Role::User, "prompt one");
    app.append_message(Role::Assistant, "reply one");
    app.append_message(Role::ToolOutput, "line one\nline two");

    app.ensure_rendered_lines(48, None);
    let before_lines = app.rendered_line_count();

    let idx = app.append_message(
        Role::Assistant,
        "tail reply with enough text to wrap across multiple transcript rows for cache testing",
    );
    assert_eq!(app.render_cache.transcript_dirty_from, Some(idx));

    app.ensure_rendered_lines(48, None);

    let expected = build_rendered_lines_with_hidden(&app.messages, 48, None);
    assert_eq!(
        rendered_signature(&app.snapshot_rendered_lines()),
        rendered_signature(&expected)
    );
    assert_eq!(
        app.render_cache.rendered_message_blocks.len(),
        app.messages.len()
    );
    assert_eq!(
        app.render_cache.rendered_block_line_counts.len(),
        app.messages.len()
    );
    assert_eq!(
        app.render_cache.rendered_block_offsets.len(),
        app.messages.len()
    );
    assert_eq!(app.render_cache.transcript_dirty_from, None);
    assert!(app.rendered_line_count() > before_lines);
}

#[test]
fn ensure_rendered_lines_incremental_agent_delta_matches_full_rebuild() {
    let mut app = AppState::new("thread".to_string());
    app.append_message(Role::User, "prompt one");
    let idx = app.append_message(Role::Assistant, "starting answer");
    app.put_agent_item_mapping("item-1", idx);

    app.ensure_rendered_lines(52, None);
    app.upsert_agent_delta(
        "item-1",
        "\ncontinued answer with more text so the last message grows substantially",
    );
    assert_eq!(app.render_cache.transcript_dirty_from, Some(idx));

    app.ensure_rendered_lines(52, None);

    let expected = build_rendered_lines_with_hidden(&app.messages, 52, None);
    assert_eq!(
        rendered_signature(&app.snapshot_rendered_lines()),
        rendered_signature(&expected)
    );
    assert_eq!(
        app.render_cache.rendered_message_blocks.len(),
        app.messages.len()
    );
    assert_eq!(
        app.render_cache.rendered_block_line_counts.len(),
        app.messages.len()
    );
    assert_eq!(
        app.render_cache.rendered_block_offsets.len(),
        app.messages.len()
    );
    assert_eq!(app.render_cache.transcript_dirty_from, None);
}

#[test]
fn ensure_rendered_lines_non_user_fence_counts_match_rendered_block() {
    let mut app = AppState::new("thread".to_string());
    app.append_message(
        Role::ToolOutput,
        "before\n```text\nliteral fence payload that should remain visible\n```\nafter",
    );

    app.ensure_rendered_lines(48, None);

    let expected = build_rendered_lines_with_hidden(&app.messages, 48, None);
    assert_eq!(app.rendered_line_count(), expected.len());
    assert_eq!(
        rendered_signature(&app.snapshot_rendered_lines()),
        rendered_signature(&expected)
    );
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
fn compute_input_layout_wraps_input_at_word_boundaries() {
    let mut app = AppState::new("thread-1".to_string());
    let _ = app.input.insert_str("alpha beta gamma");

    let layout = compute_input_layout(
        &app,
        TerminalSize {
            width: MSG_CONTENT_X + 1 + 10,
            height: 8,
        },
    );

    assert_eq!(layout.visible_lines, vec!["alpha beta", "gamma"]);
    assert_eq!(layout.input_height, 2);
    assert_eq!(layout.cursor_x, MSG_CONTENT_X + 5);
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

#[test]
fn rewind_fork_drops_selected_message_and_newer_history() {
    let mut app = AppState::new("thread-1".to_string());
    let u1 = app.append_message(Role::User, "first");
    app.record_input_history("first", Some(u1));
    let _ = app.append_message(Role::Assistant, "reply one");
    let u2 = app.append_message(Role::User, "second");
    app.record_input_history("second", Some(u2));
    let _ = app.append_message(Role::Assistant, "reply two");

    app.rewind_fork_from_message_idx(Some(u2));

    assert_eq!(app.messages.len(), u2);
    assert!(app.messages.iter().all(|m| m.text != "second"));
    assert!(app.messages.iter().all(|m| m.text != "reply two"));
    assert_eq!(app.input_history_message_indices(), &[Some(u1), None]);
}

#[test]
fn rewind_edit_keeps_selected_anchor_for_fork() {
    let mut app = AppState::new("thread-1".to_string());
    let u1 = app.append_message(Role::User, "first");
    app.record_input_history("first", Some(u1));
    let _ = app.append_message(Role::Assistant, "reply one");
    let u2 = app.append_message(Role::User, "second");
    app.record_input_history("second", Some(u2));
    let _ = app.append_message(Role::Assistant, "reply two");

    app.enter_rewind_mode();
    let _ = app.navigate_input_history_up(); // select first
    assert_eq!(app.rewind_selected_message_idx(), Some(u1));

    app.input_insert_text(" edited".to_string()); // mutate text while rewinding
    assert_eq!(app.rewind_selected_message_idx(), Some(u1));

    app.rewind_fork_from_message_idx(app.rewind_selected_message_idx());
    assert_eq!(app.messages.len(), u1);
    assert!(app.messages.iter().all(|m| m.text != "first"));
    assert!(app.messages.iter().all(|m| m.text != "reply one"));
}

#[test]
fn record_input_history_backfills_pending_message_index() {
    let mut app = AppState::new("thread-1".to_string());
    app.push_input_history("prompt");
    assert_eq!(app.input_history_message_indices(), &[None]);

    app.record_input_history("prompt", Some(42));
    assert_eq!(app.input_history_len(), 1);
    assert_eq!(app.input_history_message_indices(), &[Some(42)]);
}

#[test]
fn rewind_scroll_aligns_to_selected_prompt_history_position() {
    let mut app = AppState::new("thread-1".to_string());
    let u1 = app.append_message(Role::User, "first");
    app.record_input_history("first", Some(u1));
    let _ = app.append_message(Role::Assistant, "reply one");
    let u2 = app.append_message(Role::User, "second");
    app.record_input_history("second", Some(u2));
    let _ = app.append_message(Role::Assistant, "reply two");
    let _ = app.append_message(Role::Assistant, "tail");

    app.set_rewind_selection_for_test(Some(1));
    app.align_rewind_scroll_to_selected_prompt(TerminalSize {
        width: 100,
        height: 6,
    });
    let newer_scroll = app.scroll_top;

    app.set_rewind_selection_for_test(Some(0));
    app.align_rewind_scroll_to_selected_prompt(TerminalSize {
        width: 100,
        height: 6,
    });
    let older_scroll = app.scroll_top;

    assert!(older_scroll <= newer_scroll);
}

#[test]
fn build_rendered_lines_hides_markdown_fence_delimiters() {
    let messages = vec![Message {
        role: Role::Assistant,
        text: "```zig\nconst x = 1;\n```\n".to_string(),
        kind: MessageKind::Plain,
        file_path: None,
    }];

    let rendered = build_rendered_lines(&messages, 60);
    assert!(rendered.iter().all(|l| !l.text.contains("```")));
}

#[test]
fn build_rendered_lines_user_code_block_adds_spacing_and_background() {
    let messages = vec![Message {
        role: Role::User,
        text: "before\n```rust\nlet x = 1;\n```\nafter".to_string(),
        kind: MessageKind::Plain,
        file_path: None,
    }];

    let rendered = build_rendered_lines(&messages, 120);
    let code_idx = rendered
        .iter()
        .position(|l| l.text == "let x = 1;")
        .expect("expected code line");
    assert!(rendered.iter().all(|l| !l.text.contains("```")));
    assert_eq!(rendered[code_idx - 1].cells, 0);
    assert_eq!(rendered[code_idx + 1].cells, 0);
    assert!(rendered[code_idx]
        .styled_segments
        .iter()
        .any(|s| s.style.bg == Some(COLOR_STEP2)));
}

#[test]
fn build_rendered_lines_styles_assistant_code_lines() {
    let messages = vec![Message {
        role: Role::Assistant,
        text: "```rust\nfn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n```".to_string(),
        kind: MessageKind::Plain,
        file_path: None,
    }];

    let rendered = build_rendered_lines(&messages, 120);
    let line = rendered
        .iter()
        .find(|l| l.text.contains("fn add"))
        .expect("expected highlighted code line");

    assert!(!line.styled_segments.is_empty());
    assert!(line
        .styled_segments
        .iter()
        .any(|s| s.style != Style::default()));
}

#[test]
fn build_rendered_lines_reasoning_uses_markdown_text_without_markers() {
    let messages = vec![Message {
        role: Role::Reasoning,
        text: "**Committing cleanup with style**".to_string(),
        kind: MessageKind::Plain,
        file_path: None,
    }];

    let rendered = build_rendered_lines(&messages, 120);
    assert_eq!(rendered.len(), 1);
    assert_eq!(rendered[0].text, "Committing cleanup with style");
    assert!(!rendered[0].text.contains("Thinking:"));
    assert!(!rendered[0].text.contains("**"));
}

#[test]
fn build_rendered_lines_skips_empty_placeholders_and_their_separators() {
    let messages = vec![
        Message {
            role: Role::Assistant,
            text: String::new(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            role: Role::Reasoning,
            text: "   ".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            role: Role::Assistant,
            text: "visible".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
    ];

    let rendered = build_rendered_lines(&messages, 120);
    assert_eq!(rendered.len(), 1);
    assert_eq!(rendered[0].text, "visible");
    assert!(!rendered[0].separator);
}

#[test]
fn build_rendered_lines_renders_commentary_as_preamble() {
    let messages = vec![Message {
        role: Role::Commentary,
        text: "checking the diff".to_string(),
        kind: MessageKind::Plain,
        file_path: None,
    }];

    let rendered = build_rendered_lines(&messages, 120);
    assert_eq!(rendered.len(), 1);
    assert_eq!(rendered[0].text, "checking the diff");
}

#[test]
fn build_rendered_lines_groups_commentary_with_following_tool_call() {
    let messages = vec![
        Message {
            role: Role::Commentary,
            text: "checking the diff".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            role: Role::ToolCall,
            text: "→ Read src/main.rs".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
    ];

    let rendered = build_rendered_lines(&messages, 120);
    assert_eq!(rendered.len(), 3);
    assert_eq!(rendered[0].text, "checking the diff");
    assert!(rendered[1].separator);
    assert_eq!(rendered[2].text, "→ Read src/main.rs");
}

#[test]
fn build_rendered_lines_tool_output_multiline_has_no_indent() {
    let messages = vec![Message {
        role: Role::ToolOutput,
        text: "$ rg -n foo src/main.rs\n1:fn a()\n2:fn b()".to_string(),
        kind: MessageKind::Plain,
        file_path: None,
    }];

    let rendered = build_rendered_lines(&messages, 120);
    assert_eq!(rendered.len(), 3);
    assert_eq!(rendered[0].text, "$ rg -n foo src/main.rs");
    assert_eq!(rendered[1].text, "1:fn a()");
    assert_eq!(rendered[2].text, "2:fn b()");
}

#[test]
fn extract_diff_blocks_reads_nested_metadata_files() {
    let item = json!({
        "type": "toolOutput",
        "metadata": {
            "files": [
                {
                    "filePath": "src/main.rs",
                    "diff": "@@ -1,1 +1,1 @@\n-old\n+new\n"
                }
            ]
        }
    });

    let blocks = extract_diff_blocks(&item);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].file_path.as_deref(), Some("src/main.rs"));
    assert!(blocks[0].diff.contains("@@ -1,1 +1,1 @@"));
}

#[test]
fn build_rendered_lines_diff_styles_added_and_removed_lines() {
    let messages = vec![Message {
        role: Role::ToolOutput,
        text: "@@ -1,1 +1,1 @@\n-old\n+new\n".to_string(),
        kind: MessageKind::Diff,
        file_path: Some("src/main.rs".to_string()),
    }];

    let rendered = build_rendered_lines(&messages, 120);
    let removed = rendered
        .iter()
        .find(|l| l.text.contains("-old"))
        .expect("missing removed line");
    let added = rendered
        .iter()
        .find(|l| l.text.contains("+new"))
        .expect("missing added line");

    assert!(removed
        .styled_segments
        .iter()
        .any(|s| s.style.fg == Some(COLOR_DIFF_REMOVE)));
    assert!(added
        .styled_segments
        .iter()
        .any(|s| s.style.fg == Some(COLOR_DIFF_ADD)));
}

#[test]
fn build_rendered_lines_diff_viewer_hides_raw_hunk_headers() {
    let messages = vec![Message {
        role: Role::ToolOutput,
        text: "@@ -1,1 +1,1 @@\n-old\n+new\n@@ -10,1 +10,1 @@\n-foo\n+bar\n".to_string(),
        kind: MessageKind::Diff,
        file_path: Some("src/main.rs".to_string()),
    }];

    let rendered = build_rendered_lines(&messages, 120);
    assert!(rendered.iter().any(|l| l.text.starts_with("Hunk 1/2")));
    assert!(rendered.iter().any(|l| l.text.starts_with("Hunk 2/2")));
    let file_path_rows = rendered.iter().filter(|l| l.text == "src/main.rs").count();
    assert_eq!(file_path_rows, 2);
    assert!(!rendered
        .iter()
        .any(|l| l.text.trim_start().starts_with("@@ -")));
}

#[test]
fn build_rendered_lines_diff_viewer_wraps_long_hunk_body_lines() {
    let messages = vec![Message {
        role: Role::ToolOutput,
        text: concat!(
            "@@ -164,1 +185,1 @@\n",
            "-No decisions recorded yet\n",
            "+- Decision: EP012 must generate the per-episode labeled extractor inputs itself if they already exist.\n",
        )
        .to_string(),
        kind: MessageKind::Diff,
        file_path: Some(".agents/execplans/EXECPLAN_012_qwen_customvoice_dataset_export.md".to_string()),
    }];

    let rendered = build_rendered_lines(&messages, 80);
    assert!(rendered.iter().any(|l| l.text.contains("Hunk 1/1")));
    let wrapped_parts: Vec<&RenderedLine> = rendered
        .iter()
        .filter(|l| l.text.contains("Decision: EP012") || l.text.contains("already exist."))
        .collect();
    assert!(wrapped_parts.len() >= 2);
    assert!(wrapped_parts.iter().all(|l| l.cells <= 80));
}

#[test]
fn count_rendered_block_for_diff_matches_materialized_block_len() {
    let msg = Message {
        role: Role::ToolOutput,
        text: "diff --git a/src/example.rs b/src/example.rs\n@@ -1,2 +1,3 @@\n old\n-old line\n+new line\n+extra line\n"
            .to_string(),
        kind: MessageKind::Diff,
        file_path: Some("src/example.rs".to_string()),
    };

    let count = count_rendered_block_for_message(None, &msg, 80);
    let block = build_rendered_block_for_message(None, &msg, 80);
    assert_eq!(count, block.len());
}

#[test]
fn count_rendered_block_for_plain_tool_output_matches_materialized_block_len() {
    let msg = Message {
        role: Role::ToolOutput,
        text: concat!(
            "short line\n",
            "This is a deliberately long ASCII line that should wrap at natural word boundaries when the width is narrow enough to require multiple rendered rows in the transcript view.\n",
            "\n",
            "trailing spaces stay visible   \n",
            "averyveryveryveryveryveryveryveryveryveryveryveryveryveryveryveryveryveryveryveryveryverylongtoken\n",
        )
        .to_string(),
        kind: MessageKind::Plain,
        file_path: None,
    };

    let count = count_rendered_block_for_message(None, &msg, 48);
    let block = build_rendered_block_for_message(None, &msg, 48);
    assert_eq!(count, block.len());
}

#[test]
fn count_rendered_block_for_ansi_tool_output_matches_materialized_block_len() {
    let msg = Message {
        role: Role::ToolOutput,
        text: "\u{1b}[31mcolored status\u{1b}[0m\nplain tail\n".to_string(),
        kind: MessageKind::Plain,
        file_path: None,
    };

    let count = count_rendered_block_for_message(None, &msg, 48);
    let block = build_rendered_block_for_message(None, &msg, 48);
    assert_eq!(count, block.len());
}

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

#[test]
fn prioritize_events_drains_deferred_server_lines_without_new_input() {
    let mut deferred = std::collections::VecDeque::new();
    deferred.push_back("a".to_string());
    deferred.push_back("b".to_string());
    deferred.push_back("c".to_string());

    let prioritized = prioritize_events(Vec::new(), &mut deferred, 2);
    assert_eq!(prioritized.len(), 2);
    assert!(matches!(prioritized[0], UiEvent::ServerLine(ref s) if s == "a"));
    assert!(matches!(prioritized[1], UiEvent::ServerLine(ref s) if s == "b"));
    assert_eq!(deferred.len(), 1);
    assert_eq!(deferred.front().map(String::as_str), Some("c"));
}

#[test]
fn prioritize_events_promotes_turn_completed_even_when_budget_is_zero() {
    let mut deferred = std::collections::VecDeque::new();
    deferred.push_back("low-1".to_string());
    deferred.push_back("{\"method\":\"turn/completed\",\"params\":{}}".to_string());
    deferred.push_back("low-2".to_string());

    let prioritized = prioritize_events(Vec::new(), &mut deferred, 0);
    assert_eq!(prioritized.len(), 1);
    assert!(matches!(
        prioritized[0],
        UiEvent::ServerLine(ref s) if s.contains("\"method\":\"turn/completed\"")
    ));
    assert_eq!(deferred.len(), 2);
    assert_eq!(deferred.front().map(String::as_str), Some("low-1"));
}

#[test]
fn is_priority_server_line_identifies_control_notifications() {
    assert!(is_priority_server_line(
        "{\"method\":\"turn/completed\",\"params\":{}}"
    ));
    assert!(is_priority_server_line(
        "{\"method\":\"turn/started\",\"params\":{}}"
    ));
    assert!(is_priority_server_line(
        "{\"method\":\"error\",\"params\":{}}"
    ));
    assert!(!is_priority_server_line(
        "{\"method\":\"item/completed\",\"params\":{}}"
    ));
    assert!(!is_priority_server_line("not-json"));
}

#[test]
fn context_usage_label_formats_k_and_percent() {
    let label = context_usage_label(ContextUsage {
        used: 128_000,
        max: 256_000,
    });
    assert_eq!(label, "128k/256k (50%)");
}

#[test]
fn context_label_reserved_cells_uses_fixed_minimum() {
    assert_eq!(
        context_label_reserved_cells(None),
        visual_width("999k/999k (99%)")
    );
}

#[test]
fn context_label_reserved_cells_expands_for_longer_labels() {
    let label = "12345k/12345k (100%)";
    assert_eq!(
        context_label_reserved_cells(Some(label)),
        visual_width(label)
    );
}

#[test]
fn handle_notification_updates_context_usage_when_present() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"turn/completed\",\"params\":{\"usage\":{\"context_window\":256000,\"context_tokens\":128000}}}",
        );

    assert_eq!(app.context_usage, None);
}

#[test]
fn handle_notification_turn_completed_interrupted_appends_system_message() {
    let mut app = AppState::new("thread-1".to_string());
    app.active_turn_id = Some("turn-1".to_string());

    handle_notification_line(
        &mut app,
        "{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thread-1\",\"turn\":{\"id\":\"turn-1\",\"status\":\"interrupted\"}}}",
    );

    assert_eq!(app.active_turn_id, None);
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::System);
    assert_eq!(app.messages[0].text, "Turn interrupted");
}

#[test]
fn incoming_agent_delta_does_not_reenable_auto_follow_when_scrolled_up() {
    let mut app = AppState::new("thread-1".to_string());
    let idx = app.append_message(Role::Assistant, "hello");
    app.put_agent_item_mapping("item-1", idx);
    app.auto_follow_bottom = false;
    app.scroll_top = 4;

    handle_notification_line(
        &mut app,
        "{\"method\":\"item/agentMessage/delta\",\"params\":{\"itemId\":\"item-1\",\"delta\":\" world\"}}",
    );

    assert!(!app.auto_follow_bottom);
    assert_eq!(app.scroll_top, 4);
    assert_eq!(app.messages[idx].text, "hello world");
}

#[test]
fn handle_server_request_command_execution_sets_pending_approval() {
    let mut app = AppState::new("thread-1".to_string());

    let action = handle_server_message_line(
        &mut app,
        "{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"item/commandExecution/requestApproval\",\"params\":{\"itemId\":\"item-1\",\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"command\":\"git diff -- src/main.rs\",\"cwd\":\"/repo\",\"reason\":\"needs write access\",\"availableDecisions\":[\"accept\",\"acceptForSession\",\"decline\",\"cancel\"]}}",
    );

    assert!(action.is_none());
    let pending = app.approval.pending.expect("pending approval");
    assert_eq!(pending.method, "item/commandExecution/requestApproval");
    assert_eq!(pending.title, "Approve command execution");
    assert_eq!(pending.detail_lines[0], "git diff -- src/main.rs");
    assert!(pending.can_accept_for_session);
    assert!(pending.can_decline);
    assert!(pending.can_cancel);
}

#[test]
fn permissions_approval_response_allows_turn_or_session_grant() {
    let request = super::state::PendingApprovalRequest {
        request_id: json!("req-2"),
        method: "item/permissions/requestApproval".to_string(),
        kind: super::state::ApprovalRequestKind::Permissions,
        title: "Grant additional permissions".to_string(),
        detail_lines: vec!["network access".to_string()],
        requested_permissions: Some(json!({"network":{"enabled":true}})),
        can_accept_for_session: true,
        can_decline: true,
        can_cancel: false,
    };

    assert_eq!(
        request.response_for_choice(super::state::ApprovalChoice::Accept),
        Some(json!({"permissions":{"network":{"enabled":true}}}))
    );
    assert_eq!(
        request.response_for_choice(super::state::ApprovalChoice::AcceptForSession),
        Some(json!({"permissions":{"network":{"enabled":true}},"scope":"session"}))
    );
    assert_eq!(
        request.response_for_choice(super::state::ApprovalChoice::Decline),
        Some(json!({"permissions":{}}))
    );
}

#[test]
fn unsupported_server_request_returns_jsonrpc_error_action() {
    let mut app = AppState::new("thread-1".to_string());

    let action = handle_server_message_line(
        &mut app,
        "{\"jsonrpc\":\"2.0\",\"id\":99,\"method\":\"item/tool/requestUserInput\",\"params\":{\"itemId\":\"item-1\"}}",
    );

    match action {
        Some(ServerRequestAction::ReplyError {
            request_id,
            code,
            message,
        }) => {
            assert_eq!(request_id, json!(99));
            assert_eq!(code, -32601);
            assert!(message.contains("unsupported server request"));
        }
        _ => panic!("expected reply error"),
    }
    assert!(app.approval.pending.is_none());
}

#[test]
fn append_turn_interrupted_marker_is_deduplicated() {
    let mut app = AppState::new("thread-1".to_string());
    app.append_turn_interrupted_marker();
    app.append_turn_interrupted_marker();
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::System);
    assert_eq!(app.messages[0].text, "Turn interrupted");
}

#[test]
fn handle_notification_thread_token_usage_updated_sets_context_usage() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"thread/tokenUsage/updated\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"tokenUsage\":{\"modelContextWindow\":256000,\"last\":{\"cachedInputTokens\":0,\"inputTokens\":0,\"outputTokens\":0,\"reasoningOutputTokens\":0,\"totalTokens\":0},\"total\":{\"cachedInputTokens\":0,\"inputTokens\":100000,\"outputTokens\":22000,\"reasoningOutputTokens\":0,\"totalTokens\":122000}}}}",
        );

    assert_eq!(
        app.context_usage,
        Some(ContextUsage {
            used: 122_000,
            max: 256_000
        })
    );
}

#[test]
fn handle_notification_thread_token_usage_prefers_last_when_total_exceeds_window() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"thread/tokenUsage/updated\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"tokenUsage\":{\"modelContextWindow\":258400,\"last\":{\"cachedInputTokens\":0,\"inputTokens\":46000,\"outputTokens\":300,\"reasoningOutputTokens\":37,\"totalTokens\":46337},\"total\":{\"cachedInputTokens\":0,\"inputTokens\":301000,\"outputTokens\":2104,\"reasoningOutputTokens\":30,\"totalTokens\":303134}}}}",
        );

    assert_eq!(
        app.context_usage,
        Some(ContextUsage {
            used: 46_337,
            max: 258_400
        })
    );
}

#[test]
fn handle_notification_codex_event_token_count_sets_context_usage() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/token_count\",\"params\":{\"id\":\"turn-1\",\"msg\":{\"info\":{\"total_token_usage\":{\"input_tokens\":8815,\"cached_input_tokens\":0,\"output_tokens\":41,\"reasoning_output_tokens\":0,\"total_tokens\":8856},\"last_token_usage\":{\"input_tokens\":8815,\"cached_input_tokens\":0,\"output_tokens\":41,\"reasoning_output_tokens\":0,\"total_tokens\":8856},\"model_context_window\":258400,\"cost_usd\":0.0045096,\"total_cost_usd\":0.0045096}}}}",
        );

    assert_eq!(
        app.context_usage,
        Some(ContextUsage {
            used: 8_856,
            max: 258_400
        })
    );
}

#[test]
fn handle_notification_token_count_prefers_last_when_total_exceeds_window() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/token_count\",\"params\":{\"id\":\"turn-1\",\"msg\":{\"info\":{\"total_token_usage\":{\"input_tokens\":301030,\"cached_input_tokens\":264832,\"output_tokens\":2104,\"reasoning_output_tokens\":906,\"total_tokens\":303134},\"last_token_usage\":{\"input_tokens\":46249,\"cached_input_tokens\":43648,\"output_tokens\":88,\"reasoning_output_tokens\":56,\"total_tokens\":46337},\"model_context_window\":258400}}}}",
        );

    assert_eq!(
        app.context_usage,
        Some(ContextUsage {
            used: 46_337,
            max: 258_400
        })
    );
}

#[test]
fn handle_notification_codex_event_token_count_ignores_null_info() {
    let mut app = AppState::new("thread-1".to_string());
    app.context_usage = Some(ContextUsage {
        used: 10_000,
        max: 256_000,
    });
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/token_count\",\"params\":{\"id\":\"turn-1\",\"msg\":{\"info\":null}}}",
        );

    assert_eq!(
        app.context_usage,
        Some(ContextUsage {
            used: 10_000,
            max: 256_000
        })
    );
}

#[test]
fn load_history_does_not_seed_context_usage_from_start_response() {
    let mut app = AppState::new("thread-1".to_string());
    let response = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "thread": {
                "id": "thread-1",
                "tokenUsage": {
                    "modelContextWindow": 256000,
                    "last": {
                        "cachedInputTokens": 0,
                        "inputTokens": 0,
                        "outputTokens": 0,
                        "reasoningOutputTokens": 0,
                        "totalTokens": 0
                    },
                    "total": {
                        "cachedInputTokens": 0,
                        "inputTokens": 80000,
                        "outputTokens": 12000,
                        "reasoningOutputTokens": 0,
                        "totalTokens": 92000
                    }
                },
                "turns": []
            }
        }
    })
    .to_string();

    load_history_from_start_or_resume(&mut app, &response).expect("load history");
    assert_eq!(app.context_usage, None);
}

#[test]
fn load_history_seeds_input_history_from_user_messages() {
    let mut app = AppState::new("thread-1".to_string());
    let response = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "thread": {
                "id": "thread-1",
                "turns": [
                    {
                        "items": [
                            {
                                "type": "userMessage",
                                "content": [
                                    {"type": "text", "text": "first message"}
                                ]
                            },
                            {
                                "type": "agentMessage",
                                "text": "reply"
                            },
                            {
                                "type": "userMessage",
                                "content": [
                                    {"type": "text", "text": "second message"}
                                ]
                            }
                        ]
                    }
                ]
            }
        }
    })
    .to_string();

    load_history_from_start_or_resume(&mut app, &response).expect("load history");
    assert!(app.navigate_input_history_up());
    assert_eq!(app.input_text(), "second message");
    assert!(app.navigate_input_history_up());
    assert_eq!(app.input_text(), "first message");
}

#[test]
fn handle_notification_thread_compacted_appends_system_marker() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"thread/compacted\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::System);
    assert_eq!(app.messages[0].text, "↻ Context compacted");
}

#[test]
fn handle_notification_item_completed_context_compaction_appends_marker() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"contextCompaction\",\"id\":\"ctx-1\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::System);
    assert_eq!(app.messages[0].text, "↻ Context compacted");
}

#[test]
fn append_history_from_thread_includes_context_compaction_marker() {
    let mut app = AppState::new("thread-1".to_string());
    let thread = json!({
        "turns": [
            {
                "items": [
                    {
                        "type": "contextCompaction",
                        "id": "ctx-1"
                    }
                ]
            }
        ]
    });

    append_history_from_thread(&mut app, &thread);
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::System);
    assert_eq!(app.messages[0].text, "↻ Context compacted");
}

#[test]
fn append_history_from_thread_preserves_agent_commentary_phase() {
    let mut app = AppState::new("thread-1".to_string());
    let thread = json!({
        "turns": [
            {
                "items": [
                    {
                        "type": "agentMessage",
                        "id": "a-1",
                        "phase": "commentary",
                        "text": "checking the diff"
                    },
                    {
                        "type": "agentMessage",
                        "id": "a-2",
                        "phase": "final",
                        "text": "done"
                    }
                ]
            }
        ]
    });

    append_history_from_thread(&mut app, &thread);
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[0].role, Role::Commentary);
    assert_eq!(app.messages[0].text, "checking the diff");
    assert_eq!(app.messages[1].role, Role::Assistant);
    assert_eq!(app.messages[1].text, "done");
}

#[test]
fn append_history_from_thread_reads_reasoning_summary_text_objects() {
    let mut app = AppState::new("thread-1".to_string());
    let thread = json!({
        "turns": [
            {
                "items": [
                    {
                        "type": "reasoning",
                        "id": "r-1",
                        "summary": [
                            { "type": "summary_text", "text": "**Plan**\n\nParagraph" },
                            { "type": "summary_text", "text": "**Check**" }
                        ]
                    }
                ]
            }
        ]
    });

    append_history_from_thread(&mut app, &thread);
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::Reasoning);
    assert_eq!(app.messages[0].text, "**Plan**\n\nParagraph\n**Check**");
}

#[test]
fn handle_notification_turn_diff_updated_upserts_diff_message() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"turn/diff/updated\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"diff\":\"diff --git a/test.txt b/test.txt\\n@@ -1 +1 @@\\n-old\\n+new\\n\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::ToolOutput);
    assert_eq!(app.messages[0].kind, MessageKind::Diff);
    assert!(app.messages[0].text.contains("+new"));

    handle_notification_line(
            &mut app,
            "{\"method\":\"turn/diff/updated\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"diff\":\"diff --git a/test.txt b/test.txt\\n@@ -1 +1 @@\\n-old\\n+newer\\n\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert!(app.messages[0].text.contains("+newer"));
}

#[test]
fn handle_notification_item_completed_reasoning_replaces_live_delta_with_summary_objects() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"reasoning\",\"id\":\"reason-1\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    app.upsert_reasoning_summary_delta("reason-1", "**Designing fullscreen focus overrides ");
    app.upsert_reasoning_summary_delta("reason-1", "**");

    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"reasoning\",\"id\":\"reason-1\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"**Designing fullscreen focus overrides**\\n\\nI’m working out a helper function.\"},{\"type\":\"summary_text\",\"text\":\"**Analyzing input handling conditions**\"}]},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(
        app.messages[0].text,
        "**Designing fullscreen focus overrides**\n\nI’m working out a helper function.\n**Analyzing input handling conditions**"
    );

    let rendered = build_rendered_lines(&app.messages, 120);
    assert_eq!(rendered[0].text, "Designing fullscreen focus overrides");
    assert_eq!(rendered[1].text, "I’m working out a helper function.");
    assert_eq!(rendered[2].text, "Analyzing input handling conditions");
}

#[test]
fn handle_notification_codex_event_turn_diff_adds_diff_message() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/turn_diff\",\"params\":{\"id\":\"turn-2\",\"msg\":{\"type\":\"turn_diff\",\"unified_diff\":\"diff --git a/a b/a\\n@@ -1 +1 @@\\n-a\\n+b\\n\"}}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].kind, MessageKind::Diff);
    assert!(app.messages[0].text.contains("+b"));
}

#[test]
fn handle_notification_raw_function_call_renders_tool_call() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/raw_response_item\",\"params\":{\"msg\":{\"item\":{\"type\":\"function_call\",\"name\":\"exec_command\",\"call_id\":\"call_1\",\"arguments\":\"{\\\"cmd\\\":\\\"cat test.txt\\\"}\"}}}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::ToolCall);
    assert!(app.messages[0].text.contains("run `cat test.txt`"));
}

#[test]
fn handle_notification_raw_function_call_output_updates_existing_call() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/raw_response_item\",\"params\":{\"msg\":{\"item\":{\"type\":\"function_call\",\"name\":\"exec_command\",\"call_id\":\"call_2\",\"arguments\":\"{\\\"cmd\\\":\\\"echo hi\\\"}\"}}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/raw_response_item\",\"params\":{\"msg\":{\"item\":{\"type\":\"function_call_output\",\"call_id\":\"call_2\",\"output\":\"hi\\n\"}}}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::ToolOutput);
    assert_eq!(app.messages[0].kind, MessageKind::Plain);
    assert!(app.messages[0].text.contains("hi"));
}

#[test]
fn handle_notification_raw_function_call_output_diff_is_rendered_as_diff() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/raw_response_item\",\"params\":{\"msg\":{\"item\":{\"type\":\"function_call_output\",\"call_id\":\"call_3\",\"output\":\"diff --git a/x b/x\\n@@ -1 +1 @@\\n-old\\n+new\\n\"}}}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::ToolOutput);
    assert_eq!(app.messages[0].kind, MessageKind::Diff);
    assert!(app.messages[0].text.contains("+new"));
}

#[test]
fn raw_function_call_dedupes_with_command_execution_item_started() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/raw_response_item\",\"params\":{\"msg\":{\"item\":{\"type\":\"function_call\",\"name\":\"exec_command\",\"call_id\":\"call_4\",\"arguments\":\"{\\\"cmd\\\":\\\"pwd\\\"}\"}}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_4\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
}

#[test]
fn item_started_agent_message_uses_commentary_role_when_phase_present() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"agentMessage\",\"id\":\"msg-1\",\"phase\":\"commentary\",\"text\":\"\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::Commentary);
}

#[test]
fn exec_command_end_read_override_suppresses_large_read_output_on_success() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_read_1\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_read_1\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"read\",\"cmd\":\"cat src/main.rs\",\"name\":\"main.rs\",\"path\":\"/repo/src/main.rs\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_read_1\",\"aggregatedOutput\":\"line1\\nline2\\nline3\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].text, "→ Read src/main.rs");
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn exec_command_end_search_override_suppresses_large_output_on_success() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_1\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_search_1\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"search\",\"cmd\":\"rg -n foo src/main.rs\",\"path\":\"/repo/src/main.rs\",\"pattern\":\"foo\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_1\",\"aggregatedOutput\":\"1:fn a()\\n2:fn b()\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].text, "✱ Search src/main.rs [pattern=foo]");
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn exec_command_end_search_override_kept_on_error_with_output() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_err\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_search_err\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"search\",\"cmd\":\"rg -n foo src/main.rs\",\"path\":\"/repo/src/main.rs\",\"pattern\":\"foo\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_err\",\"command\":\"/usr/bin/zsh -lc 'rg -n foo src/main.rs missing.rs'\",\"aggregatedOutput\":\"rg: missing.rs: No such file or directory\\n\",\"exitCode\":2},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].role, Role::ToolOutput);
    assert!(app.messages[0]
        .text
        .starts_with("✱ Search src/main.rs [pattern=foo]\n$ rg -n foo src/main.rs missing.rs"));
    assert!(app.messages[0]
        .text
        .contains("rg: missing.rs: No such file or directory"));
    assert!(app.messages[0].text.contains("exit code: 2"));
}

#[test]
fn exec_command_end_generic_shell_nl_sed_is_summarized_as_search() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_read_nl\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_read_nl\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"shell\",\"cmd\":\"nl -ba src/main.rs | sed -n '3398,3465p'\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_read_nl\",\"aggregatedOutput\":\"3398 abc\\n3399 def\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search src/main.rs [lines=3398..3465]"
    );
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn exec_command_end_generic_shell_rg_is_summarized_as_search() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_search_rg\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"shell\",\"cmd\":\"rg -n \\\"count|build\\\\(\\\" src/app/render.rs\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg\",\"aggregatedOutput\":\"916:build_rendered_block_for_message\\n927:count_rendered_block_for_message\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search src/app/render.rs [pattern=count|build\\(]"
    );
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn exec_command_end_generic_shell_sed_is_summarized_as_search() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_sed\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_search_sed\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"shell\",\"cmd\":\"sed -n '916,1015p' src/app/render.rs\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_sed\",\"aggregatedOutput\":\"pub(super) fn build_rendered_block_for_message(\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search src/app/render.rs [lines=916..1015]"
    );
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn item_completed_shell_rg_falls_back_to_search_summary_without_exec_command_end() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg_fallback\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg_fallback\",\"command\":\"rg -n 'bundles:|runtime:|full:|bin:|misc:' pkg/core/userland/util_linux.yaml\",\"aggregatedOutput\":\"95:bundles:\\n102:  full:\\n111:  runtime:\\n116:  bin:\\n912:  misc:\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search pkg/core/userland/util_linux.yaml [pattern=bundles:|runtime:|full:|bin:|misc:]"
    );
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn item_completed_shell_sed_falls_back_to_search_summary_without_exec_command_end() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_sed_fallback\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_sed_fallback\",\"command\":\"sed -n '95,125p' pkg/core/userland/util_linux.yaml\",\"aggregatedOutput\":\"bundles:\\n  dev:\\n  - bin\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search pkg/core/userland/util_linux.yaml [lines=95..125]"
    );
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn item_completed_shell_nl_sed_pipeline_falls_back_to_search_summary_without_exec_command_end() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_nl_sed_fallback\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_nl_sed_fallback\",\"command\":\"nl -ba /home/wegel/work/tt/projects/soniq/wegel/apps/tools/rootfs-builder/meta-soniq/recipes-multimedia/wireplumber/wireplumber_%.bbappend | sed -n '1,120p'\",\"aggregatedOutput\":\"     1\\tFILESEXTRAPATHS:prepend := \\\"${THISDIR}/files:\\\"\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search /home/wegel/work/tt/projects/soniq/wegel/apps/tools/rootfs-builder/meta-soniq/recipes-multimedia/wireplumber/wireplumber_%.bbappend [lines=1..120]"
    );
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn item_completed_shell_rg_multi_path_falls_back_to_search_summary_without_exec_command_end() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg_multi_fallback\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );
    handle_notification_line(
        &mut app,
        "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_search_rg_multi_fallback\",\"command\":\"rg -n 'bash_completion|bash-completion' asm pkg\",\"aggregatedOutput\":\"asm/foo:1:...\\npkg/bar:2:...\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
    );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(
        app.messages[0].text,
        "✱ Search asm pkg [pattern=bash_completion|bash-completion]"
    );
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn exec_command_end_shell_git_diff_is_summarized_as_diff() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_git_diff\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_git_diff\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"shell\",\"cmd\":\"git diff -- src/main.rs\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_git_diff\",\"aggregatedOutput\":\"diff --git a/src/main.rs b/src/main.rs\\n@@ -1 +1 @@\\n-old\\n+new\\n\",\"exitCode\":1},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].kind, MessageKind::Diff);
}

#[test]
fn exec_command_end_parsed_edit_type_is_summarized_as_edit() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_edit_1\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/exec_command_end\",\"params\":{\"msg\":{\"type\":\"exec_command_end\",\"call_id\":\"call_edit_1\",\"cwd\":\"/repo\",\"parsed_cmd\":[{\"type\":\"edit\",\"path\":\"/repo/src/main.rs\"}]}}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_edit_1\",\"aggregatedOutput\":\"patched\\n\",\"exitCode\":0},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].text, "← Edit src/main.rs");
    assert_eq!(app.messages[0].role, Role::ToolCall);
}

#[test]
fn item_completed_command_execution_diff_output_renders_diff_message_kind() {
    let mut app = AppState::new("thread-1".to_string());
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_diff_1\"},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );
    handle_notification_line(
            &mut app,
            "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"commandExecution\",\"id\":\"call_diff_1\",\"aggregatedOutput\":\"diff --git a/test.txt b/test.txt\\n@@ -1 +1 @@\\n-old\\n+new\\n\",\"exitCode\":1},\"threadId\":\"thread-1\",\"turnId\":\"turn-1\"}}",
        );

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].kind, MessageKind::Diff);
    assert!(app.messages[0].text.contains("+new"));
}

#[test]
fn format_tool_item_run_style_from_command_fields() {
    let item = json!({
        "type": "toolCall",
        "input": {
            "command": "cargo test",
            "reasoning": "Running cargo test in repo",
            "description": "Runs Rust test suite using Cargo"
        }
    });

    let rendered = format_tool_item(&item, Role::ToolCall).expect("formatted tool call");
    assert!(rendered.contains("run `cargo test`"));
    assert!(rendered.contains("Thinking: Running cargo test in repo"));
    assert!(rendered.contains("# Runs Rust test suite using Cargo"));
    assert!(rendered.contains("$ cargo test"));
}

#[test]
fn format_tool_item_collects_stdout_stderr_and_exit_code() {
    let item = json!({
        "type": "toolOutput",
        "stdout": "Finished `test` profile [optimized + debuginfo] target(s) in 0.04s",
        "stderr": "",
        "exitCode": 0
    });

    let rendered = format_tool_item(&item, Role::ToolOutput).expect("formatted output");
    assert!(rendered.contains("Finished `test` profile"));
    assert!(rendered.contains("exit code: 0"));
}

#[test]
fn format_tool_item_read_call_shows_offset_bracket() {
    let item = json!({
        "type": "toolCall",
        "tool": "read",
        "input": {
            "filePath": "src/main.rs",
            "offset": 1791
        }
    });

    let rendered = format_tool_item(&item, Role::ToolCall).expect("formatted read call");
    assert_eq!(rendered, "→ Read src/main.rs [offset=1791]");
}

#[test]
fn format_command_execution_call_uses_action_command() {
    let item = json!({
        "type": "commandExecution",
        "id": "call_1",
        "command": "/usr/bin/zsh -lc 'ls -1'",
        "commandActions": [
            { "type": "listFiles", "command": "ls -1", "path": null }
        ],
        "status": "inProgress"
    });

    let rendered = format_tool_item(&item, Role::ToolCall).expect("formatted command call");
    assert_eq!(rendered, "run `ls -1`\n$ ls -1");
}

#[test]
fn parse_ssh_remote_command_extracts_destination_and_payload() {
    let parsed = parse_ssh_remote_command(
        "ssh -i /tmp/key -o BatchMode=yes wegel@192.168.3.20 'curl -fsS http://127.0.0.1:29091/admin/v1/jobs/123 | jq'",
    )
    .expect("parsed ssh command");

    assert_eq!(parsed.destination, "wegel@192.168.3.20");
    assert_eq!(
        parsed.remote_command,
        "curl -fsS http://127.0.0.1:29091/admin/v1/jobs/123 | jq"
    );
}

#[test]
fn strip_terminal_controls_preserving_sgr_removes_control_noise() {
    let cleaned = strip_terminal_controls_preserving_sgr(
        "\u{1b}[7l\u{1b}[31mhello\u{1b}[0m\n\u{1b}]0;title\u{07}world\u{1b}[?25h",
    );
    assert_eq!(cleaned, "\u{1b}[31mhello\u{1b}[0m\nworld");
}

#[test]
fn strip_terminal_controls_preserving_sgr_handles_unknown_escape_before_unicode() {
    let cleaned = strip_terminal_controls_preserving_sgr("\u{1b}��g��a��������");
    assert_eq!(cleaned, "��g��a��������");
}

#[test]
fn strip_terminal_controls_removes_sgr_sequences_too() {
    let cleaned =
        strip_terminal_controls("\u{1b}[7l\u{1b}[31mhello\u{1b}[0m\n\u{1b}]0;title\u{07}world");
    assert_eq!(cleaned, "hello\nworld");
}

#[test]
fn format_command_execution_call_rewrites_ssh_transport_as_remote_exec() {
    let item = json!({
        "type": "commandExecution",
        "id": "call_ssh_1",
        "command": "ssh -i /tmp/key -o BatchMode=yes wegel@192.168.3.20 'curl -fsS http://127.0.0.1:29091/admin/v1/jobs/123 | jq'",
        "status": "inProgress"
    });

    let rendered = format_tool_item(&item, Role::ToolCall).expect("formatted command call");
    assert_eq!(
        rendered,
        "remote exec on wegel@192.168.3.20\n$ curl -fsS http://127.0.0.1:29091/admin/v1/jobs/123 | jq"
    );
}

#[test]
fn format_command_execution_output_uses_aggregated_output() {
    let item = json!({
        "type": "commandExecution",
        "id": "call_1",
        "command": "/usr/bin/zsh -lc 'ls -1'",
        "commandActions": [
            { "type": "listFiles", "command": "ls -1", "path": null }
        ],
        "aggregatedOutput": "a\nb\n",
        "exitCode": 0,
        "durationMs": 51,
        "status": "completed"
    });

    let rendered = format_tool_item(&item, Role::ToolOutput).expect("formatted command output");
    assert!(rendered.starts_with("$ ls -1\n"), "rendered={rendered:?}");
    assert!(rendered.contains("a\nb"), "rendered={rendered:?}");
    assert!(rendered.contains("exit code: 0"), "rendered={rendered:?}");
}

#[test]
fn format_command_execution_output_preserves_sgr_sequences_but_strips_control_noise() {
    let item = json!({
        "type": "commandExecution",
        "id": "call_ansi_1",
        "command": "/usr/bin/zsh -lc 'printf hello'",
        "aggregatedOutput": "\u{001b}[7l\u{001b}[31mhello\u{001b}[0m\n",
        "exitCode": 0,
        "status": "completed"
    });

    let rendered = format_tool_item(&item, Role::ToolOutput).expect("formatted output");
    assert_eq!(
        rendered,
        "$ printf hello\n\u{001b}[31mhello\u{001b}[0m\n\nexit code: 0"
    );
}

#[test]
fn format_command_execution_output_rewrites_ssh_transport_as_remote_exec() {
    let item = json!({
        "type": "commandExecution",
        "id": "call_ssh_2",
        "command": "ssh -i /tmp/key -o BatchMode=yes wegel@192.168.3.20 'curl -fsS http://127.0.0.1:29091/admin/v1/jobs/123 | jq'",
        "aggregatedOutput": "{\n  \"state\": \"running\"\n}\n",
        "exitCode": 0,
        "status": "completed"
    });

    let rendered = format_tool_item(&item, Role::ToolOutput).expect("formatted output");
    assert_eq!(
        rendered,
        "remote exec on wegel@192.168.3.20\n$ curl -fsS http://127.0.0.1:29091/admin/v1/jobs/123 | jq\n{\n  \"state\": \"running\"\n}\n\nexit code: 0"
    );
}

#[test]
fn build_rendered_lines_tool_output_uses_ansi_styles_without_escape_text() {
    let messages = vec![Message {
        role: Role::ToolOutput,
        text: "$ printf hello\n\u{001b}[31mhello\u{001b}[0m".to_string(),
        kind: MessageKind::Plain,
        file_path: None,
    }];

    let rendered = build_rendered_lines(&messages, 120);
    assert_eq!(rendered.len(), 2);
    assert_eq!(rendered[0].text, "$ printf hello");
    assert_eq!(rendered[1].text, "hello");
    assert!(!rendered[1].text.contains('\u{001b}'));
    assert!(rendered[1]
        .styled_segments
        .iter()
        .any(|seg| seg.style.fg.is_some() || !seg.style.add_modifier.is_empty()));
}

#[test]
fn reasoning_summary_delta_inserts_newline_between_bold_chunks() {
    let mut app = AppState::new("thread-1".to_string());
    let idx = app.append_message(Role::Reasoning, String::new());
    app.put_agent_item_mapping("reason-1", idx);

    app.upsert_reasoning_summary_delta("reason-1", "**First thought**");
    app.upsert_reasoning_summary_delta("reason-1", "**Second thought**");

    assert_eq!(
        app.messages[idx].text,
        "**First thought**\n**Second thought**"
    );
}

#[test]
fn reasoning_summary_delta_trims_space_before_split_closing_marker() {
    let mut app = AppState::new("thread-1".to_string());
    let idx = app.append_message(Role::Reasoning, String::new());
    app.put_agent_item_mapping("reason-1", idx);

    app.upsert_reasoning_summary_delta("reason-1", "**Preparing concise, evidence-based answer ");
    app.upsert_reasoning_summary_delta("reason-1", "**");

    assert_eq!(
        app.messages[idx].text,
        "**Preparing concise, evidence-based answer**"
    );
}

#[test]
fn reasoning_summary_delta_normalizes_split_adjacent_bold_chunks() {
    let mut app = AppState::new("thread-1".to_string());
    let idx = app.append_message(Role::Reasoning, String::new());
    app.put_agent_item_mapping("reason-1", idx);

    app.upsert_reasoning_summary_delta("reason-1", "**Investigating display bug causes ");
    app.upsert_reasoning_summary_delta("reason-1", "**");
    app.upsert_reasoning_summary_delta("reason-1", " **Analyzing markdown rendering issues ");
    app.upsert_reasoning_summary_delta("reason-1", "**");

    assert_eq!(
        app.messages[idx].text,
        "**Investigating display bug causes**\n**Analyzing markdown rendering issues**"
    );

    let rendered = build_rendered_lines(&app.messages, 120);
    assert_eq!(rendered.len(), 2);
    assert_eq!(rendered[0].text, "Investigating display bug causes");
    assert_eq!(rendered[1].text, "Analyzing markdown rendering issues");
}

#[test]
fn selected_text_preserves_reasoning_heading_and_paragraph_breaks() {
    let messages = vec![Message {
        role: Role::Reasoning,
        text: "**Planning final-form migration implementation**\n\nI’m preparing to inspect the repo and data layout to understand the migration state.\n**Deciding on one-time migration approach**".to_string(),
        kind: MessageKind::Plain,
        file_path: None,
    }];

    let rendered = build_rendered_lines(&messages, 120);
    let last_idx = rendered.len() - 1;
    let sel = Selection {
        anchor_x: 1,
        anchor_line_idx: 0,
        focus_x: rendered[last_idx].cells,
        focus_line_idx: last_idx,
        dragging: false,
    };

    let out = selected_text(sel, &rendered);
    assert_eq!(
        out,
        "Planning final-form migration implementation\nI’m preparing to inspect the repo and data layout to understand the migration state.\nDeciding on one-time migration approach"
    );
}

#[test]
fn params_turn_interrupt_includes_thread_and_turn_id() {
    let params = params_turn_interrupt("thread-1", "turn-9");
    assert_eq!(
        params,
        json!({
            "threadId": "thread-1",
            "turnId": "turn-9"
        })
    );
}

#[test]
fn command_execution_diff_output_detects_unified_diff() {
    let item = json!({
        "type": "commandExecution",
        "aggregatedOutput": "diff --git a/x b/x\n@@ -1 +1 @@\n-old\n+new\n"
    });
    let diff = command_execution_diff_output(&item).expect("should detect diff");
    assert!(diff.contains("+new"));
}

#[test]
fn widechar_selection_uses_cell_offsets() {
    let line = "a😀b";
    assert_eq!(visual_width(line), 4);
    let s = slice_by_cells(line, 1, 3);
    assert_eq!(s, "😀");
}

#[test]
fn osc52_wrap_detects_tmux_and_screen() {
    assert_eq!(
        detect_osc52_wrap(Some("/tmp/tmux-1000/default,123,0"), Some("xterm-256color")),
        Osc52Wrap::Tmux
    );
    assert_eq!(
        detect_osc52_wrap(None, Some("screen-256color")),
        Osc52Wrap::Screen
    );
    assert_eq!(
        detect_osc52_wrap(None, Some("xterm-256color")),
        Osc52Wrap::None
    );
}

#[test]
fn osc52_tmux_sequence_uses_passthrough_and_escaped_esc() {
    let encoded = "YQ==";
    let seqs = osc52_sequences_for_env(encoded, Some("1"), Some("xterm-256color"));
    let first = &seqs[0];
    assert!(first.starts_with("\x1bPtmux;"));
    assert!(first.contains("\x1b\x1b]52;c;YQ=="));
    assert!(first.ends_with("\x1b\\"));
}

#[test]
fn osc52_screen_sequence_uses_dcs_wrapper() {
    let encoded = "YQ==";
    let seqs = osc52_sequences_for_env(encoded, None, Some("screen-256color"));
    let first = &seqs[0];
    assert!(first.starts_with("\x1bP\x1b]52;c;YQ=="));
    assert!(first.ends_with("\x1b\\"));
}

#[test]
fn osc52_generates_both_clipboard_targets() {
    let seqs = osc52_sequences_for_env("YQ==", None, Some("xterm-256color"));
    assert!(seqs.iter().any(|s| s.contains("]52;c;YQ==")));
    assert!(seqs.iter().any(|s| s.contains("]52;p;YQ==")));
}

#[test]
fn ssh_detection_works() {
    assert!(is_ssh_session(Some("/dev/pts/3"), None, None));
    assert!(is_ssh_session(None, Some("1.2.3.4 22 5.6.7.8 54321"), None));
    assert!(is_ssh_session(None, None, Some("1.2.3.4 54321 22")));
    assert!(!is_ssh_session(None, None, None));
}

#[test]
fn normalize_pasted_text_converts_crlf_and_cr() {
    let text = "a\r\nb\rc";
    assert_eq!(normalize_pasted_text(text), "a\nb\nc");
}

#[test]
fn is_newline_enter_accepts_shift_and_alt() {
    assert!(is_newline_enter(KeyModifiers::SHIFT));
    assert!(is_newline_enter(KeyModifiers::ALT));
    assert!(!is_newline_enter(KeyModifiers::empty()));
}

#[test]
fn is_key_press_like_accepts_repeat() {
    assert!(is_key_press_like(KeyEventKind::Press));
    assert!(is_key_press_like(KeyEventKind::Repeat));
    assert!(!is_key_press_like(KeyEventKind::Release));
}

#[test]
fn decide_mouse_drag_mode_prefers_scroll_for_vertical_swipe() {
    assert_eq!(
        decide_mouse_drag_mode(10, 10, 10, 14),
        MouseDragMode::Scroll
    );
    assert_eq!(
        decide_mouse_drag_mode(10, 10, 12, 14),
        MouseDragMode::Select
    );
    assert_eq!(
        decide_mouse_drag_mode(10, 10, 10, 11),
        MouseDragMode::Select
    );
    assert_eq!(
        decide_mouse_drag_mode(10, 10, 10, 10),
        MouseDragMode::Undecided
    );
}

#[test]
fn parse_mobile_mouse_coords_accepts_plain_and_sgr_fragments() {
    assert_eq!(parse_mobile_mouse_coords("76;46"), Some((76, 46)));
    assert_eq!(
        parse_mobile_mouse_coords("\u{1b}[<64;76;46M"),
        Some((76, 46))
    );
    assert_eq!(parse_mobile_mouse_coords("hello"), None);
}

#[test]
fn consume_mobile_mouse_char_does_not_swallow_plain_digits() {
    let mut app = AppState::new("thread-1".to_string());
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, '2'),
        MobileMouseConsume::PassThrough
    ));
    assert!(app.mobile_mouse_buffer.is_empty());
}

#[test]
fn consume_mobile_mouse_char_requires_prefix_to_activate() {
    let mut app = AppState::new("thread-1".to_string());
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, '<'),
        MobileMouseConsume::Consumed
    ));
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, '7'),
        MobileMouseConsume::Consumed
    ));
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, '6'),
        MobileMouseConsume::Consumed
    ));
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, ';'),
        MobileMouseConsume::Consumed
    ));
}

#[test]
fn consume_mobile_mouse_char_accepts_csi_bracket_prefix() {
    let mut app = AppState::new("thread-1".to_string());
    app.scroll_top = 12;
    app.mobile_mouse_last_y = Some(40);

    for ch in ['[', '<', '6', '4', ';', '7', '6', ';', '4', '6', 'M'] {
        assert!(matches!(
            consume_mobile_mouse_char(&mut app, ch),
            MobileMouseConsume::Consumed
        ));
    }
    assert_eq!(app.scroll_top, 18);
    assert!(app.mobile_mouse_buffer.is_empty());
}

#[test]
fn mobile_mouse_key_candidate_accepts_alt_prefixed_csi_chars() {
    let app = AppState::new("thread-1".to_string());
    assert!(is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char('['),
        KeyModifiers::ALT
    ));
    assert!(is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char('7'),
        KeyModifiers::ALT
    ));
    assert!(!is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char('7'),
        KeyModifiers::empty()
    ));
}

#[test]
fn mobile_mouse_key_candidate_accepts_plain_coords_when_pending() {
    let mut app = AppState::new("thread-1".to_string());
    app.mobile_plain_pending_coords = true;
    assert!(is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char('7'),
        KeyModifiers::empty()
    ));
    assert!(is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char(';'),
        KeyModifiers::empty()
    ));
    assert!(!is_mobile_mouse_key_candidate(
        &app,
        KeyCode::Char('a'),
        KeyModifiers::empty()
    ));
}

#[test]
fn consume_mobile_mouse_char_plain_pending_pair_applies_scroll() {
    let mut app = AppState::new("thread-1".to_string());
    app.mobile_plain_pending_coords = true;
    app.scroll_top = 20;
    app.mobile_mouse_last_y = Some(50);

    for ch in ['6', '6', ';', '5', '2'] {
        assert!(matches!(
            consume_mobile_mouse_char(&mut app, ch),
            MobileMouseConsume::Consumed
        ));
    }

    assert_eq!(app.scroll_top, 23);
    assert!(!app.mobile_plain_pending_coords);
    assert!(app.mobile_mouse_buffer.is_empty());
}

#[test]
fn consume_mobile_mouse_char_plain_pending_repeated_pair_reuses_direction() {
    let mut app = AppState::new("thread-1".to_string());
    app.scroll_top = 20;
    app.mobile_mouse_last_y = Some(50);

    app.mobile_plain_pending_coords = true;
    for ch in ['6', '6', ';', '5', '2'] {
        let _ = consume_mobile_mouse_char(&mut app, ch);
    }
    assert_eq!(app.scroll_top, 23);

    app.mobile_plain_pending_coords = true;
    for ch in ['6', '6', ';', '5', '2'] {
        let _ = consume_mobile_mouse_char(&mut app, ch);
    }
    assert_eq!(app.scroll_top, 26);
}

#[test]
fn consume_mobile_mouse_char_plain_pending_new_gesture_keeps_prior_direction() {
    let mut app = AppState::new("thread-1".to_string());
    app.scroll_top = 20;
    app.mobile_mouse_last_y = Some(50);
    app.mobile_plain_last_direction = 1;
    app.mobile_plain_new_gesture = true;
    app.mobile_plain_pending_coords = true;

    for ch in ['6', '4', ';', '4', '7'] {
        let _ = consume_mobile_mouse_char(&mut app, ch);
    }

    assert_eq!(app.scroll_top, 23);
    assert_eq!(app.mobile_plain_last_direction, 1);
}

#[test]
fn parse_repeated_plain_mobile_pair_accepts_concatenated_repetition() {
    assert_eq!(
        parse_repeated_plain_mobile_pair("75;4375;4375;43"),
        Some((75, 43))
    );
    assert_eq!(
        parse_repeated_plain_mobile_pair("71;5371;5371;53"),
        Some((71, 53))
    );
    assert_eq!(parse_repeated_plain_mobile_pair("75;43"), None);
}

#[test]
fn consume_mobile_mouse_char_applies_scroll_on_terminator() {
    let mut app = AppState::new("thread-1".to_string());
    app.scroll_top = 10;
    app.mobile_mouse_last_y = Some(40);

    for ch in ['<', '6', '4', ';', '7', '6', ';', '4', '6'] {
        assert!(matches!(
            consume_mobile_mouse_char(&mut app, ch),
            MobileMouseConsume::Consumed
        ));
    }
    assert!(matches!(
        consume_mobile_mouse_char(&mut app, 'M'),
        MobileMouseConsume::Consumed
    ));
    assert_eq!(app.scroll_top, 16);
    assert!(app.mobile_mouse_buffer.is_empty());
}

#[test]
fn apply_mobile_mouse_scroll_uses_natural_touch_direction() {
    let mut app = AppState::new("thread-1".to_string());
    app.scroll_top = 20;
    app.mobile_mouse_last_y = Some(40);

    apply_mobile_mouse_scroll(&mut app, 44);
    assert_eq!(app.scroll_top, 24);

    apply_mobile_mouse_scroll(&mut app, 42);
    assert_eq!(app.scroll_top, 22);
}

#[test]
fn apply_mobile_mouse_scroll_honors_invert_toggle() {
    let mut app = AppState::new("thread-1".to_string());
    app.scroll_inverted = true;
    app.scroll_top = 20;
    app.mobile_mouse_last_y = Some(40);

    apply_mobile_mouse_scroll(&mut app, 44);
    assert_eq!(app.scroll_top, 16);

    apply_mobile_mouse_scroll(&mut app, 42);
    assert_eq!(app.scroll_top, 18);
}

#[test]
fn kitt_head_index_bounces_across_separator() {
    let seq: Vec<usize> = (0..9).map(|tick| kitt_head_index(5, tick)).collect();
    assert_eq!(seq, vec![0, 1, 2, 3, 4, 3, 2, 1, 0]);
    assert_eq!(kitt_head_index(1, 42), 0);
}

#[test]
fn draw_rendered_line_renders_uncovered_styled_tail() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 8, 1));
    let line = RenderedLine {
        text: "suite.".to_string(),
        styled_segments: vec![StyledSegment {
            text: "suite".to_string(),
            style: Style::default().fg(COLOR_TEXT),
        }],
        role: Role::Assistant,
        separator: false,
        cells: visual_width("suite."),
        soft_wrap_to_next: false,
    };

    draw_rendered_line(&mut buf, 0, 0, 8, &line, Style::default(), None);
    assert_eq!(buf[(5, 0)].symbol(), ".");
}

#[test]
fn draw_picker_delete_dialog_shows_prompt_and_session_id() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 72, 20));
    let target = ThreadSummary {
        id: "thread-123".to_string(),
        name: Some("Parser follow-up".to_string()),
        preview: "preview".to_string(),
        cwd: "/repo".to_string(),
        created_at: 1,
        updated_at: 2,
    };

    draw_picker_delete_dialog(
        &mut buf,
        TerminalSize {
            width: 72,
            height: 20,
        },
        &target,
    );

    let rendered = (0..20)
        .map(|y| (0..72).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Delete session?"));
    assert!(rendered.contains("Parser follow-up"));
    assert!(rendered.contains("thread-123"));
    assert!(rendered.contains("y/Enter delete  n/Esc cancel"));
}

#[test]
fn normalize_styled_segments_for_part_falls_back_on_mismatch() {
    let styled = vec![StyledSegment {
        text: "visua".to_string(),
        style: Style::default().fg(COLOR_PRIMARY),
    }];
    let normalized = normalize_styled_segments_for_part("visual.", styled);
    assert_eq!(normalized.len(), 1);
    assert_eq!(normalized[0].text, "visual.");
    assert_eq!(normalized[0].style, Style::default());
}

#[test]
fn duration_samples_tracks_percentiles_and_window() {
    let mut samples = DurationSamples::new(3);
    samples.push(Duration::from_micros(1_000));
    samples.push(Duration::from_micros(2_000));
    samples.push(Duration::from_micros(3_000));
    samples.push(Duration::from_micros(4_000));

    // Window size is 3, so first sample is dropped.
    assert_eq!(samples.values_us.len(), 3);
    assert_eq!(samples.percentile_us(0.50), Some(3_000));
    assert_eq!(samples.percentile_us(0.95), Some(4_000));
    assert_eq!(samples.avg_ms(), 2.50);
    assert_eq!(samples.max_ms(), 4.0);
}

#[test]
fn perf_metrics_overlay_lines_include_latency_rows() {
    let mut perf = PerfMetrics::new();
    perf.poll_wait.push(Duration::from_micros(500));
    perf.event_handle.push(Duration::from_micros(700));
    perf.draw.push(Duration::from_micros(900));
    perf.key_interval.push(Duration::from_micros(1_100));
    perf.repeat_interval.push(Duration::from_micros(1_300));
    perf.press_to_first_repeat
        .push(Duration::from_micros(1_500));
    perf.release_to_next_key.push(Duration::from_micros(1_700));

    let lines = perf.overlay_lines();
    assert!(lines.iter().any(|line| line.contains("poll wait")));
    assert!(lines.iter().any(|line| line.contains("event handle")));
    assert!(lines.iter().any(|line| line.contains("draw")));
    assert!(lines.iter().any(|line| line.contains("key interval")));
    assert!(lines.iter().any(|line| line.contains("repeat intvl")));
    assert!(lines.iter().any(|line| line.contains("press->repeat")));
    assert!(lines.iter().any(|line| line.contains("release->key")));
}

#[test]
fn perf_metrics_tracks_repeat_transition_buckets() {
    let mut perf = PerfMetrics::new();
    perf.mark_key_kind(KeyEventKind::Press);
    perf.mark_key_kind(KeyEventKind::Repeat);
    perf.mark_key_kind(KeyEventKind::Repeat);
    perf.mark_key_kind(KeyEventKind::Release);
    perf.mark_key_kind(KeyEventKind::Press);

    assert_eq!(perf.key_press_events, 2);
    assert_eq!(perf.key_repeat_events, 2);
    assert_eq!(perf.key_release_events, 1);
    assert_eq!(perf.press_to_first_repeat.values_us.len(), 1);
    assert_eq!(perf.repeat_interval.values_us.len(), 1);
    assert_eq!(perf.release_to_next_key.values_us.len(), 1);
}

#[test]
fn parse_cli_args_supports_ralph_resume_and_markers() {
    let args = vec![
        "resume".to_string(),
        "session-123".to_string(),
        "--ralph-prompt".to_string(),
        "custom/prompt.md".to_string(),
        "--ralph-done-marker".to_string(),
        "DONE".to_string(),
        "--ralph-blocked-marker".to_string(),
        "BLOCKED".to_string(),
    ];
    let parsed = parse_cli_args(args).expect("parse");

    assert!(parsed.mode_resume);
    assert_eq!(parsed.resume_id.as_deref(), Some("session-123"));
    assert_eq!(
        parsed.ralph_prompt_path.as_deref(),
        Some("custom/prompt.md")
    );
    assert_eq!(parsed.ralph_done_marker.as_deref(), Some("DONE"));
    assert_eq!(parsed.ralph_blocked_marker.as_deref(), Some("BLOCKED"));
}

#[test]
fn parse_cli_args_supports_continue_mode() {
    let args = vec!["continue".to_string()];
    let parsed = parse_cli_args(args).expect("parse");

    assert!(parsed.mode_continue);
    assert!(!parsed.mode_resume);
    assert!(!parsed.mode_perf_session);
    assert_eq!(parsed.resume_id, None);
}

#[test]
fn parse_cli_args_supports_perf_session_mode() {
    let args = vec![
        "perf-session".to_string(),
        "/tmp/session.jsonl".to_string(),
        "--width".to_string(),
        "200".to_string(),
        "--height".to_string(),
        "60".to_string(),
    ];
    let parsed = parse_cli_args(args).expect("parse");

    assert!(parsed.mode_perf_session);
    assert!(!parsed.mode_resume);
    assert_eq!(
        parsed.perf_session_path.as_deref(),
        Some("/tmp/session.jsonl")
    );
    assert_eq!(parsed.perf_width, 200);
    assert_eq!(parsed.perf_height, 60);
}

#[test]
fn parse_cli_args_supports_synthetic_perf_session_mode() {
    let args = vec![
        "perf-session".to_string(),
        "--synthetic".to_string(),
        "--turns".to_string(),
        "250".to_string(),
        "--seed".to_string(),
        "17".to_string(),
        "--tool-lines".to_string(),
        "40".to_string(),
    ];
    let parsed = parse_cli_args(args).expect("parse");

    assert!(parsed.mode_perf_session);
    assert!(parsed.perf_synthetic);
    assert_eq!(parsed.perf_session_path, None);
    assert_eq!(parsed.perf_turns, 250);
    assert_eq!(parsed.perf_seed, 17);
    assert_eq!(parsed.perf_tool_lines, 40);
}

#[test]
fn synthetic_perf_messages_are_reproducible() {
    let spec = perf_session::SyntheticPerfSpec {
        seed: 7,
        turns: 12,
        tool_output_lines: 8,
    };
    let left = perf_session::build_synthetic_perf_messages(spec);
    let right = perf_session::build_synthetic_perf_messages(spec);

    assert_eq!(left.len(), spec.turns * 6);
    assert_eq!(left.len(), right.len());
    for (lhs, rhs) in left.iter().zip(right.iter()) {
        assert_eq!(lhs.role, rhs.role);
        assert_eq!(lhs.kind, rhs.kind);
        assert_eq!(lhs.text, rhs.text);
        assert_eq!(lhs.file_path, rhs.file_path);
    }
}

#[test]
fn synthetic_perf_messages_change_with_seed() {
    let base = perf_session::build_synthetic_perf_messages(perf_session::SyntheticPerfSpec {
        seed: 7,
        turns: 4,
        tool_output_lines: 4,
    });
    let changed = perf_session::build_synthetic_perf_messages(perf_session::SyntheticPerfSpec {
        seed: 8,
        turns: 4,
        tool_output_lines: 4,
    });

    assert_eq!(base.len(), changed.len());
    assert!(base
        .iter()
        .zip(changed.iter())
        .any(|(lhs, rhs)| lhs.text != rhs.text));
}

#[test]
fn parse_thread_runtime_settings_reads_model_and_effort() {
    let response = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "thread": { "id": "thread-1" },
            "model": "gpt-5-codex",
            "reasoningEffort": "high",
            "summary": "auto"
        }
    })
    .to_string();

    let settings = parse_thread_runtime_settings(&response).expect("settings");
    assert_eq!(settings.model.as_deref(), Some("gpt-5-codex"));
    assert_eq!(settings.effort.as_deref(), Some("high"));
    assert_eq!(settings.summary.as_deref(), Some("auto"));
}

#[test]
fn parse_thread_runtime_settings_accepts_effort_alias() {
    let response = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "thread": { "id": "thread-1" },
            "model": "gpt-5.4",
            "effort": "medium"
        }
    })
    .to_string();

    let settings = parse_thread_runtime_settings(&response).expect("settings");
    assert_eq!(settings.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(settings.effort.as_deref(), Some("medium"));
}

#[test]
fn params_turn_start_includes_model_effort_and_summary_when_set() {
    let params = params_turn_start(
        "thread-1",
        "hello",
        Some("gpt-5"),
        Some("high"),
        Some("auto"),
    );
    assert_eq!(params["threadId"], "thread-1");
    assert_eq!(params["model"], "gpt-5");
    assert_eq!(params["effort"], "high");
    assert_eq!(params["summary"], "auto");
}

#[test]
fn params_turn_start_omits_model_effort_and_summary_when_missing() {
    let params = params_turn_start("thread-1", "hello", None, None, None);
    assert!(params.get("model").is_none());
    assert!(params.get("effort").is_none());
    assert!(params.get("summary").is_none());
}

#[test]
fn params_thread_archive_includes_thread_id() {
    let params = params_thread_archive("thread-123");

    assert_eq!(params["threadId"], "thread-123");
}

#[test]
fn runtime_settings_label_shows_pending_summary_override() {
    let mut app = AppState::new("thread-1".to_string());
    app.set_runtime_settings(Some("gpt-5".to_string()), Some("high".to_string()), None);
    app.apply_default_reasoning_summary(Some("auto".to_string()));

    assert_eq!(app.runtime_settings_label(), "gpt-5/high/auto*");
}

#[test]
fn next_turn_runtime_settings_falls_back_to_current_values() {
    let mut app = AppState::new("thread-1".to_string());
    app.set_runtime_settings(
        Some("gpt-5.4".to_string()),
        Some("high".to_string()),
        Some("concise".to_string()),
    );

    assert_eq!(
        app.next_turn_runtime_settings(),
        (
            Some("gpt-5.4".to_string()),
            Some("high".to_string()),
            Some("concise".to_string())
        )
    );
}

#[test]
fn next_turn_runtime_settings_prefers_pending_over_current_values() {
    let mut app = AppState::new("thread-1".to_string());
    app.set_runtime_settings(
        Some("gpt-5.4".to_string()),
        Some("high".to_string()),
        Some("concise".to_string()),
    );
    app.queue_runtime_settings(
        Some("gpt-5.4-mini".to_string()),
        Some("medium".to_string()),
        Some("auto".to_string()),
    );

    assert_eq!(
        app.next_turn_runtime_settings(),
        (
            Some("gpt-5.4-mini".to_string()),
            Some("medium".to_string()),
            Some("auto".to_string())
        )
    );
}

#[test]
fn runtime_defaults_round_trip_json_file() {
    let path = std::env::temp_dir().join(format!(
        "carlos-runtime-defaults-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let defaults = RuntimeDefaults {
        model: Some("gpt-5.4".to_string()),
        effort: Some("high".to_string()),
        summary: Some("concise".to_string()),
    };

    persist_runtime_defaults_to(&path, &defaults).expect("persist");
    let loaded = load_runtime_defaults_from(&path);
    let _ = std::fs::remove_file(&path);

    assert_eq!(loaded, defaults);
}

#[test]
fn resolve_initial_runtime_settings_falls_back_to_persisted_defaults() {
    let runtime = crate::protocol::ThreadRuntimeSettings {
        model: None,
        effort: None,
        summary: None,
    };
    let defaults = RuntimeDefaults {
        model: Some("gpt-5.4".to_string()),
        effort: Some("high".to_string()),
        summary: Some("concise".to_string()),
    };

    let resolved = resolve_initial_runtime_settings(runtime, &defaults, defaults.summary.clone());

    assert_eq!(resolved.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(resolved.effort.as_deref(), Some("high"));
    assert_eq!(resolved.summary.as_deref(), Some("concise"));
}

#[test]
fn apply_model_settings_returns_defaults_for_persistence() {
    let mut app = AppState::new("thread-1".to_string());
    app.runtime.model_settings_model_input = "gpt-5.4".to_string();
    app.runtime.model_settings_effort_options = super::state::DEFAULT_EFFORT_OPTIONS
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    app.runtime.model_settings_effort_index = 4;
    app.runtime.model_settings_summary_options = super::state::DEFAULT_SUMMARY_OPTIONS
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    app.runtime.model_settings_summary_index = 1;
    app.runtime.show_model_settings = true;

    let defaults = app.apply_model_settings();

    assert_eq!(
        defaults,
        RuntimeDefaults {
            model: Some("gpt-5.4".to_string()),
            effort: Some("high".to_string()),
            summary: Some("concise".to_string()),
        }
    );
    assert_eq!(
        app.take_pending_runtime_settings(),
        (
            Some("gpt-5.4".to_string()),
            Some("high".to_string()),
            Some("concise".to_string())
        )
    );
    assert!(!app.runtime.show_model_settings);
}

#[test]
fn detect_turn_markers_matches_trimmed_commentary_marker_lines() {
    let messages = vec![
        Message {
            role: Role::User,
            text: "@@BLOCKED@@".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            role: Role::Commentary,
            text: "working...\n  @@BLOCKED@@ \nnext".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            role: Role::Assistant,
            text: "@@COMPLETE@@".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
    ];

    let markers = super::ralph::detect_turn_markers(&messages, 0, "@@COMPLETE@@", "@@BLOCKED@@");
    assert!(markers.blocked);
    assert!(markers.completed);
}

#[test]
fn detect_turn_markers_matches_inline_marker_tokens() {
    let messages = vec![Message {
        role: Role::Assistant,
        text: "@@COMPLETE@@ @@COMPLETE_SUMMARY_START@@ summary".to_string(),
        kind: MessageKind::Plain,
        file_path: None,
    }];

    let markers = super::ralph::detect_turn_markers(&messages, 0, "@@COMPLETE@@", "@@BLOCKED@@");
    assert!(markers.completed);
    assert!(!markers.blocked);
}

#[test]
fn ralph_turn_completion_queues_continuation_when_not_blocked_or_complete() {
    let mut app = AppState::new("thread-1".to_string());
    app.enable_ralph_mode(super::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.turn_start_message_idx = Some(0);
    app.append_message(Role::Assistant, "still working");

    app.handle_ralph_turn_completed(false);

    assert!(app.has_pending_ralph_continuation());
    assert!(app.dequeue_turn_input(std::time::Instant::now()).is_none());
}

#[test]
fn ralph_turn_completion_disables_ralph_mode_on_blocked_marker() {
    let mut app = AppState::new("thread-1".to_string());
    app.enable_ralph_mode(super::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.turn_start_message_idx = Some(0);
    app.append_message(Role::Assistant, "@@BLOCKED@@");

    app.handle_ralph_turn_completed(false);

    assert!(app.queued_turn_inputs_is_empty());
    assert!(!app.ralph_enabled());
}

#[test]
fn commentary_blocked_marker_disables_ralph_mode_immediately() {
    let mut app = AppState::new("thread-1".to_string());
    app.enable_ralph_mode(super::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.turn_start_message_idx = Some(0);
    app.append_message(Role::Commentary, "@@BLOCKED@@");

    app.maybe_disable_ralph_on_blocked_marker();

    assert!(!app.ralph_enabled());
    assert!(app.messages.last().is_some_and(
        |msg| msg.role == Role::System && msg.text == "Ralph blocked: waiting for input"
    ));
}

#[test]
fn ralph_turn_completion_disables_ralph_mode() {
    let mut app = AppState::new("thread-1".to_string());
    app.enable_ralph_mode(super::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.turn_start_message_idx = Some(0);
    app.append_message(
        Role::Assistant,
        "@@COMPLETE@@ @@COMPLETE_SUMMARY_START@@".to_string(),
    );

    app.handle_ralph_turn_completed(false);

    assert!(!app.ralph_enabled());
    assert!(app.queued_turn_inputs_is_empty());
}

#[test]
fn ralph_interrupted_turn_does_not_queue_continuation() {
    let mut app = AppState::new("thread-1".to_string());
    app.enable_ralph_mode(super::ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "base".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.turn_start_message_idx = Some(0);
    app.append_message(Role::Assistant, "still working");

    app.handle_ralph_turn_completed(true);

    assert!(app.queued_turn_inputs_is_empty());
    assert!(app.ralph_enabled());
    assert!(!app.ralph_waiting_for_user());
}

#[test]
fn request_ralph_toggle_enables_and_disables_when_idle() {
    let mut app = AppState::new("thread-1".to_string());
    let tmp = std::env::temp_dir().join(format!(
        "carlos-ralph-toggle-{}.md",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("duration")
            .as_nanos()
    ));
    std::fs::write(&tmp, "auto prompt").expect("write prompt");
    app.configure_ralph_options(
        std::env::current_dir().expect("cwd"),
        Some(tmp.to_string_lossy().to_string()),
        None,
        None,
    );

    app.request_ralph_toggle().expect("toggle on");
    assert!(app.ralph_enabled());
    assert!(app.dequeue_turn_input(std::time::Instant::now()).is_some());

    app.request_ralph_toggle().expect("toggle off");
    assert!(!app.ralph_enabled());
    assert!(app.queued_turn_inputs_is_empty());

    let _ = std::fs::remove_file(tmp);
}

#[test]
fn request_ralph_toggle_defers_while_turn_active() {
    let mut app = AppState::new("thread-1".to_string());
    app.active_turn_id = Some("turn-1".to_string());

    app.request_ralph_toggle().expect("defer");
    assert!(app.ralph_toggle_pending());

    app.request_ralph_toggle().expect("cancel");
    assert!(!app.ralph_toggle_pending());
}

#[test]
fn pending_ralph_continuation_becomes_ready_after_deadline() {
    let mut app = AppState::new("thread-1".to_string());
    app.queue_ralph_continuation("continue");
    let deadline = app
        .ralph_pending_continuation_deadline()
        .expect("continuation deadline");

    assert!(app
        .dequeue_turn_input(deadline - std::time::Duration::from_millis(1))
        .is_none());
    assert_eq!(
        app.dequeue_turn_input(deadline).as_deref(),
        Some("continue")
    );
    assert!(!app.has_pending_ralph_continuation());
}

#[test]
fn parse_thread_list_reads_session_summary_fields() {
    let response = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "data": [
                {
                    "id": "thread-123",
                    "name": "Parser follow-up",
                    "preview": "Fix the failing parser test",
                    "cwd": "/repo",
                    "createdAt": 1_731_100_000,
                    "updatedAt": 1_731_200_430
                },
                {
                    "preview": "missing id should be ignored",
                    "cwd": "/repo",
                    "updatedAt": 1
                }
            ]
        }
    })
    .to_string();

    let threads = parse_thread_list(&response).expect("parse thread list");
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].id, "thread-123");
    assert_eq!(threads[0].name.as_deref(), Some("Parser follow-up"));
    assert_eq!(threads[0].preview, "Fix the failing parser test");
    assert_eq!(threads[0].cwd, "/repo");
    assert_eq!(threads[0].created_at, 1_731_100_000);
    assert_eq!(threads[0].updated_at, 1_731_200_430);
}

#[test]
fn resume_hint_formats_resume_command() {
    assert_eq!(
        resume_hint("xxxx-xxxx"),
        "to resume this session use:\ncarlos resume xxxx-xxxx"
    );
}

#[test]
fn styled_resume_hint_colors_only_command_line() {
    assert_eq!(
        styled_resume_hint("xxxx-xxxx"),
        "to resume this session use:\n\x1b[94mcarlos resume xxxx-xxxx\x1b[0m"
    );
}

#[test]
fn sort_threads_for_picker_prefers_most_recent_update_first() {
    let threads = vec![
        ThreadSummary {
            id: "older".to_string(),
            name: None,
            preview: "older".to_string(),
            cwd: "/repo".to_string(),
            created_at: 10,
            updated_at: 20,
        },
        ThreadSummary {
            id: "newer".to_string(),
            name: None,
            preview: "newer".to_string(),
            cwd: "/repo".to_string(),
            created_at: 11,
            updated_at: 30,
        },
        ThreadSummary {
            id: "same-update-later-created".to_string(),
            name: None,
            preview: "same".to_string(),
            cwd: "/repo".to_string(),
            created_at: 15,
            updated_at: 20,
        },
    ];

    let sorted = sort_threads_for_picker(&threads);
    let ids: Vec<&str> = sorted.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["newer", "same-update-later-created", "older"]);
}
