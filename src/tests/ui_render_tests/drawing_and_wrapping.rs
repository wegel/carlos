// Tests for low-level drawing primitives and cell-based wrapping.

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
