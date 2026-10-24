// Tests for markdown rendering and diff hunk display.

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
        tool_output: None,
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
        tool_output: None,
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
        tool_output: None,
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

    assert!(!line.text.contains("│"));
    assert!(!line.styled_segments.is_empty());
    assert!(line
        .styled_segments
        .iter()
        .any(|s| s.style != Style::default()));
}

#[test]
fn build_rendered_lines_renders_assistant_tables() {
    let messages = vec![Message {
        tool_output: None,
        role: Role::Assistant,
        text: "| Level | Effort |\n|---|---|\n| L0 | Low |\n| L1 | Medium |".to_string(),
        kind: MessageKind::Plain,
        file_path: None,
    }];

    let rendered = build_rendered_lines(&messages, 120);

    assert!(rendered.iter().any(|line| line.text.contains('┌')));
    assert!(rendered.iter().any(|line| line.text.contains("Level")));
    assert!(rendered.iter().any(|line| line.text.contains("L0")));
    assert!(rendered.iter().all(|line| !line.text.contains("|---|---|")));
}

#[test]
fn build_rendered_lines_reasoning_uses_markdown_text_without_markers() {
    let messages = vec![Message {
        tool_output: None,
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
            tool_output: None,
            role: Role::Assistant,
            text: String::new(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            tool_output: None,
            role: Role::Reasoning,
            text: "   ".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            tool_output: None,
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
        tool_output: None,
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
            tool_output: None,
            role: Role::Commentary,
            text: "checking the diff".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            tool_output: None,
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
        tool_output: None,
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
        tool_output: None,
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
        tool_output: None,
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
