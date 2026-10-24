//! Check source colors, diff parser state, read metadata, and unchanged text.

use ratatui::style::{Color, Style};
use serde_json::json;

use super::{
    build_rendered_block_for_message, build_rendered_lines, count_rendered_block_for_message,
    AppState, Message, MessageKind, Role,
};
use crate::app::notifications::{append_history_from_thread, handle_notification_line};
use crate::app::source_highlight::SourceHighlighter;
use crate::app::transcript_render::count_rendered_block_for_message_cached;
use crate::app::transcript_source::source_path_from_item;
use crate::app::transcript_styles::RenderCountCache;
use crate::theme::{COLOR_DIFF_ADD, COLOR_DIFF_REMOVE, COLOR_STEP2};

fn diff_message(path: Option<&str>, text: &str) -> Message {
    Message {
        tool_output: None,
        role: Role::ToolOutput,
        kind: MessageKind::Diff,
        text: text.to_string(),
        file_path: path.map(str::to_string),
    }
}

fn token_color(
    lines: &[crate::app::models::RenderedLine],
    needle: &str,
    token: &str,
) -> Option<Color> {
    let line = lines
        .iter()
        .find(|line| line.text.contains(needle))
        .expect("source line");
    line.styled_segments
        .iter()
        .find(|segment| segment.text.contains(token))
        .expect("source token")
        .style
        .fg
}

#[test]
fn source_languages_use_catppuccin_without_changing_text_or_background() {
    for (path, source) in [
        ("src/main.rs", "fn main() { let value = \"hello\"; }"),
        ("web/app.ts", "const value: string = \"hello\";"),
        ("web/app.tsx", "const element = <p>Hello</p>;"),
        ("main.py", "def hello(): return \"hello\""),
        ("Cargo.toml", "name = \"carlos\""),
        ("Dockerfile", "FROM alpine:latest"),
    ] {
        let mut highlighter = SourceHighlighter::for_path(path).expect(path);
        let spans = highlighter.highlight(source, Style::default().bg(COLOR_STEP2));
        assert_eq!(
            spans.iter().map(|s| s.text.as_str()).collect::<String>(),
            source
        );
        assert!(spans.iter().all(|s| s.style.bg == Some(COLOR_STEP2)));
        let colors: std::collections::HashSet<_> = spans.iter().map(|s| s.style.fg).collect();
        assert!(
            colors.len() > 1,
            "expected multiple token colors for {path}"
        );
    }
    assert!(SourceHighlighter::for_path("file.unknown-language").is_none());
}

#[test]
fn diff_colors_tokens_but_keeps_colored_markers_and_backgrounds() {
    let msg = diff_message(
        Some("main.rs"),
        "@@ -1 +1 @@\n-let old = \"before\";\n+let new = \"after\";\n",
    );
    let lines = build_rendered_lines(&[msg], 120);
    for (needle, marker, color) in [
        ("-let old", "-", COLOR_DIFF_REMOVE),
        ("+let new", "+", COLOR_DIFF_ADD),
    ] {
        let line = lines
            .iter()
            .find(|line| line.text.contains(needle))
            .expect("diff line");
        assert!(line
            .styled_segments
            .iter()
            .any(|s| s.text == marker && s.style.fg == Some(color)));
        assert!(line
            .styled_segments
            .iter()
            .filter(|s| s.style.bg.is_some())
            .all(|s| s.style.bg == Some(COLOR_STEP2)));
        assert_ne!(
            token_color(&lines, needle, "let"),
            token_color(&lines, needle, "\"")
        );
    }
}

#[test]
fn removed_comments_do_not_color_new_code_and_hunks_reset_parser_state() {
    let msg = diff_message(
        Some("main.rs"),
        concat!(
            "@@ -1,2 +1,2 @@\n-/* old comment\n+let added = 1;\n shared text\n",
            "@@ -20 +20 @@\n-let old = 2;\n+let reset = 3;\n",
        ),
    );
    let lines = build_rendered_lines(&[msg], 120);
    let expected = token_color(&lines, "+let added", "let");
    assert_eq!(expected, token_color(&lines, "+let reset", "let"));
    assert_eq!(expected, token_color(&lines, "-let old", "let"));
}

#[test]
fn multiline_source_comments_keep_their_color_within_each_side() {
    let msg = diff_message(
        Some("main.rs"),
        "@@ -1,3 +1,3 @@\n /* comment\n-old text\n+new text\n */\n",
    );
    let lines = build_rendered_lines(&[msg], 120);
    assert_eq!(
        token_color(&lines, "-old text", "old"),
        token_color(&lines, "+new text", "new")
    );
    assert_eq!(
        token_color(&lines, "/* comment", "comment"),
        token_color(&lines, "+new text", "new")
    );
}

#[test]
fn multi_file_diff_uses_each_filename_and_preserves_unknown_language_colors() {
    let msg = diff_message(None, concat!(
        "diff --git a/main.rs b/main.rs\n--- a/main.rs\n+++ b/main.rs\n@@ -1 +1 @@\n-old\n+let value = 1;\n",
        "diff --git a/config.toml b/config.toml\n--- a/config.toml\n+++ b/config.toml\n@@ -1 +1 @@\n-old\n+name = \"carlos\"\n",
        "diff --git a/file.unknown b/file.unknown\n--- a/file.unknown\n+++ b/file.unknown\n@@ -1 +1 @@\n-old\n+new\n",
    ));
    let lines = build_rendered_lines(&[msg], 120);
    for path in ["main.rs", "config.toml", "file.unknown"] {
        assert!(lines.iter().any(|line| line.text == path));
    }
    assert_eq!(token_color(&lines, "+new", "new"), Some(COLOR_DIFF_ADD));
    assert_ne!(
        token_color(&lines, "+let value", "let"),
        Some(COLOR_DIFF_ADD)
    );
}

#[test]
fn colored_reads_and_diffs_keep_scroll_counts_and_wrapped_text() {
    let messages = [
        diff_message(
            Some("main.rs"),
            "@@ -1 +1 @@\n-old\n+let greeting = \"héllo 🌍 and a long line of source code\";\n",
        ),
        Message {
            tool_output: None,
            role: Role::ToolOutput,
            kind: MessageKind::Plain,
            file_path: Some("main.rs".to_string()),
            text: "1:let greeting = \"héllo 🌍 and a long line of source code\";\n".to_string(),
        },
        Message {
            tool_output: None,
            role: Role::ToolOutput,
            kind: MessageKind::Plain,
            file_path: Some("notes.unknown".to_string()),
            text: "```rust\n\tcode\n```\n".to_string(),
        },
    ];
    for msg in &messages {
        for width in [8, 20, 80] {
            let lines = build_rendered_block_for_message(None, msg, width);
            assert_eq!(
                count_rendered_block_for_message(None, msg, width),
                lines.len()
            );
            assert_eq!(
                count_rendered_block_for_message_cached(
                    &mut RenderCountCache::new(),
                    None,
                    msg,
                    width
                ),
                lines.len()
            );
            for line in lines {
                assert_eq!(
                    line.styled_segments
                        .iter()
                        .map(|s| s.text.as_str())
                        .collect::<String>(),
                    line.text
                );
            }
        }
    }
}

#[test]
fn read_metadata_reaches_live_and_resumed_transcripts() {
    let item = json!({"type":"toolOutput", "id":"read-1", "name":"Read", "input":{"file_path":"main.rs"}, "output":"1:let value = \"hello\";"});
    let mut live = AppState::new("thread-1".to_string());
    handle_notification_line(
        &mut live,
        &json!({"method":"item/completed", "params":{"item":item}}).to_string(),
    );
    let mut resumed = AppState::new("thread-1".to_string());
    append_history_from_thread(&mut resumed, &json!({"turns":[{"items":[item]}]}));
    for app in [live, resumed] {
        assert_eq!(app.messages[0].file_path.as_deref(), Some("main.rs"));
        let lines = build_rendered_lines(&app.messages, 120);
        assert_eq!(lines[0].text, "1:let value = \"hello\";");
        assert_ne!(
            token_color(&lines, "let value", "let"),
            token_color(&lines, "let value", "\"")
        );
    }
}

#[test]
fn saved_command_reads_color_source_but_leave_command_and_status_plain() {
    let mut app = AppState::new("thread-1".to_string());
    append_history_from_thread(
        &mut app,
        &json!({"turns":[{"items":[{
            "type":"commandExecution", "id":"read", "command":"cat main.rs", "commandActions":[{"type":"read","command":"cat main.rs","path":"main.rs"}],
            "aggregatedOutput":"let value = \"hello\";", "exitCode":0
        }]}]}),
    );
    let lines = build_rendered_lines(&app.messages, 120);
    assert_eq!(lines[0].text, "$ cat main.rs");
    assert_eq!(lines[0].styled_segments[0].style, Style::default());
    assert_ne!(
        token_color(&lines, "let value", "let"),
        token_color(&lines, "let value", "\"")
    );
    assert_eq!(lines.last().expect("status").text, "exit code: 0");
}

#[test]
fn read_output_keeps_explicit_terminal_colors() {
    let msg = Message {
        tool_output: None,
        role: Role::ToolOutput,
        kind: MessageKind::Plain,
        file_path: Some("main.rs".to_string()),
        text: "\u{1b}[31mlet value = 1;\u{1b}[0m".to_string(),
    };
    let lines = build_rendered_lines(&[msg], 120);
    assert_eq!(lines[0].text, "let value = 1;");
    assert_eq!(lines[0].styled_segments[0].style.fg, Some(Color::Red));
}

#[test]
fn failed_reads_and_commands_with_multiple_actions_do_not_claim_one_source_file() {
    for item in [
        json!({"name":"Read", "input":{"path":"main.rs"}, "isError":true}),
        json!({"name":"Read", "input":{"path":"main.rs"}, "is_error":true}),
        json!({"name":"Read", "input":{"path":"main.rs"}, "exitCode":1}),
        json!({"commandActions":[{"type":"read","path":"main.rs"}], "stderr":"warning"}),
        json!({"commandActions":[{"type":"read","path":"main.rs"}, {"type":"read","path":"main.py"}]}),
    ] {
        assert!(source_path_from_item(&item).is_none(), "{item}");
    }
}
