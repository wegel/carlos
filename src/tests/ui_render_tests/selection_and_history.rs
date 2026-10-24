// Tests for selection handling and rewind/history rendering.

#[test]
fn render_main_view_shows_dictation_indicator() {
    let mut app = AppState::new("thread-1".to_string());
    app.configure_dictation(usable_dictation_profile());
    app.start_dictation_recording();
    let text = render_app_text(&mut app);

    assert!(text.contains("DICTATING AUTO [English]"));
}

#[test]
fn render_main_view_shows_dictation_indicator_with_ralph_label() {
    let mut app = AppState::new("thread-1".to_string());
    app.enable_ralph_mode(ralph::RalphConfig {
        prompt_path: std::path::PathBuf::from(".agents/ralph-prompt.md"),
        base_prompt: "keep working".to_string(),
        done_marker: "@@COMPLETE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        waiting_marker: "@@AWAITING_HUMAN@@".to_string(),
        continuation_prompt: "continue".to_string(),
    });
    app.configure_dictation(usable_dictation_profile());
    app.start_dictation_recording();
    let text = render_app_text(&mut app);

    assert!(text.contains("DICTATING AUTO [English]"));
    assert!(text.contains("RALPH MODE"));
}

#[test]
fn render_main_view_shows_dictation_indicator_in_rewind_mode() {
    let mut app = AppState::new("thread-1".to_string());
    app.append_message(Role::User, "old prompt");
    app.record_input_history("old prompt", Some(0));
    app.enter_rewind_mode();
    app.configure_dictation(usable_dictation_profile());
    app.start_dictation_recording();
    let text = render_app_text(&mut app);

    assert!(app.rewind_mode());
    assert!(text.contains("DICTATING AUTO [English]"));
}

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
            tool_output: None,
            role: Role::User,
            text: "first".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            tool_output: None,
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
