use super::*;

#[test]
fn compute_selection_range_normalizes_reversed_coordinates() {
    let sel = Selection {
        anchor_x: 10,
        anchor_y: 5,
        focus_x: 3,
        focus_y: 5,
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
        anchor_y: MSG_TOP,
        focus_x: 4,
        focus_y: MSG_TOP,
        dragging: false,
    };

    let out = selected_text(sel, &lines, 19, 0);
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
        anchor_y: MSG_TOP,
        focus_x: 5,
        focus_y: MSG_TOP + 1,
        dragging: false,
    };

    let out = selected_text(sel, &lines, 19, 0);
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
        anchor_y: MSG_TOP,
        focus_x: 5,
        focus_y: MSG_TOP + 1,
        dragging: false,
    };

    let out = selected_text(sel, &lines, 19, 0);
    assert_eq!(out, "abcde\nfghij");
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
fn collapse_successive_read_summaries_merges_same_file() {
    let messages = vec![
        Message {
            role: Role::ToolCall,
            text: "→ Read src/main.rs [offset=10]".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            role: Role::ToolCall,
            text: "→ Read src/main.rs".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            role: Role::ToolCall,
            text: "→ Read src/main.rs [offset=30]".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            role: Role::ToolCall,
            text: "→ Read src/lib.rs".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
    ];

    let collapsed = collapse_successive_read_summaries(&messages);
    assert_eq!(collapsed.len(), 2);
    assert_eq!(collapsed[0].text, "→ Read src/main.rs ×3");
    assert_eq!(collapsed[1].text, "→ Read src/lib.rs");
}

#[test]
fn collapse_successive_read_summaries_stops_at_non_read_message() {
    let messages = vec![
        Message {
            role: Role::ToolCall,
            text: "→ Read src/main.rs".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        },
        Message {
            role: Role::Assistant,
            text: "working".to_string(),
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

    let collapsed = collapse_successive_read_summaries(&messages);
    assert_eq!(collapsed.len(), 3);
    assert_eq!(collapsed[0].text, "→ Read src/main.rs");
    assert_eq!(collapsed[1].text, "working");
    assert_eq!(collapsed[2].text, "→ Read src/main.rs");
}

#[test]
fn wrap_natural_by_cells_prefers_word_boundaries() {
    let parts = wrap_natural_by_cells("alpha beta gamma", 10);
    assert_eq!(parts, vec!["alpha beta".to_string(), "gamma".to_string()]);
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
    assert!(!rendered
        .iter()
        .any(|l| l.text.trim_start().starts_with("@@ -")));
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
fn extract_context_usage_reads_nested_usage_fields() {
    let payload = json!({
        "result": {
            "usage": {
                "contextWindow": 256000,
                "contextTokens": 120000
            }
        },
        "params": {
            "metrics": {
                "context_window": 128000,
                "input_tokens": 64000
            }
        }
    });

    let usage = extract_context_usage(&payload).expect("context usage");
    assert_eq!(
        usage,
        ContextUsage {
            used: 120_000,
            max: 256_000
        }
    );
}

#[test]
fn extract_context_usage_reads_thread_token_usage_shape() {
    let payload = json!({
        "params": {
            "tokenUsage": {
                "modelContextWindow": 256000,
                "total": {
                    "totalTokens": 111111
                }
            }
        }
    });

    let usage = extract_context_usage(&payload).expect("context usage");
    assert_eq!(
        usage,
        ContextUsage {
            used: 111_111,
            max: 256_000
        }
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
fn exec_command_end_generic_shell_nl_sed_is_summarized_as_read() {
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
    assert_eq!(app.messages[0].text, "→ Read src/main.rs");
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
    assert!(!consume_mobile_mouse_char(&mut app, '2'));
    assert!(app.mobile_mouse_buffer.is_empty());
}

#[test]
fn consume_mobile_mouse_char_requires_prefix_to_activate() {
    let mut app = AppState::new("thread-1".to_string());
    assert!(consume_mobile_mouse_char(&mut app, '<'));
    assert!(consume_mobile_mouse_char(&mut app, '7'));
    assert!(consume_mobile_mouse_char(&mut app, '6'));
    assert!(consume_mobile_mouse_char(&mut app, ';'));
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
