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
    let newer_scroll = app.viewport.scroll_top;

    app.set_rewind_selection_for_test(Some(0));
    app.align_rewind_scroll_to_selected_prompt(TerminalSize {
        width: 100,
        height: 6,
    });
    let older_scroll = app.viewport.scroll_top;

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

