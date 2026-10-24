//! Check tool modes, reversible folding, scroll anchors, and saved preferences.

use std::fs;

use super::{AppState, Role, TerminalSize};
use crate::app::models::{MessageKind, ToolOutputDetails};
use crate::app::state::ModelSettingsField;
use crate::app::tool_output_preferences::{load_mode_from, save_mode_to};
use crate::app::tool_output_state::{ToolOutputMode, ToolOutputState};

fn size(width: usize, height: usize) -> TerminalSize {
    TerminalSize { width, height }
}

fn long_output(app: &mut AppState) -> usize {
    let text = (0..12)
        .map(|i| format!("source line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let idx = app.append_message(Role::ToolOutput, text);
    app.messages[idx].tool_output = Some(ToolOutputDetails {
        summary: "→ Read main.rs".into(),
        failed: false,
    });
    idx
}

fn screen(app: &mut AppState, width: usize) -> String {
    app.ensure_rendered_lines(width, None);
    app.snapshot_rendered_lines()
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn auto_uses_both_dimensions_and_keeps_its_choice_near_the_cutoff() {
    let mut state = ToolOutputState::default();
    state.update_size(size(80, 20));
    assert!(!state.compact());
    state.update_size(size(79, 40));
    assert!(state.compact());
    state.update_size(size(80, 40));
    assert!(
        state.compact(),
        "a one-column resize must not unfold the transcript"
    );
    state.update_size(size(84, 22));
    assert!(!state.compact());
    state.update_size(size(120, 19));
    assert!(
        state.compact(),
        "a short landscape phone terminal also needs compact output"
    );
    state.update_size(size(0, 0));
    assert!(
        state.compact(),
        "an empty size report must not change the view"
    );
}

#[test]
fn manual_modes_override_resize_until_the_user_returns_to_auto() {
    let mut state = ToolOutputState::default();
    state.mode = ToolOutputMode::Full;
    state.update_size(size(40, 12));
    assert!(!state.compact());
    state.mode = ToolOutputMode::Compact;
    state.update_size(size(160, 48));
    assert!(state.compact());
    state.mode = ToolOutputMode::Auto;
    assert!(!state.compact());
}

#[test]
fn settings_open_with_the_current_choice_and_apply_or_cancel_the_draft() {
    let mut app = AppState::new("thread".into());
    app.tool_output.mode = ToolOutputMode::Full;
    app.open_model_settings();
    app.model_settings_move_field(false);
    assert_eq!(
        app.runtime.model_settings_field,
        ModelSettingsField::ToolOutput
    );
    assert_eq!(app.runtime.model_settings_tool_output, ToolOutputMode::Full);
    app.runtime.model_settings_tool_output = ToolOutputMode::Compact;
    app.close_model_settings();
    assert_eq!(app.tool_output.mode, ToolOutputMode::Full);
    app.open_model_settings();
    assert_eq!(app.runtime.model_settings_tool_output, ToolOutputMode::Full);
    app.runtime.model_settings_tool_output = ToolOutputMode::Compact;
    app.apply_model_settings();
    assert_eq!(app.tool_output.mode, ToolOutputMode::Compact);
}

#[test]
fn settings_report_the_automatic_choice_and_arrow_keys_wrap_both_ways() {
    let mut state = ToolOutputState::default();
    state.update_size(size(40, 18));
    assert_eq!(state.settings_label(ToolOutputMode::Auto), "Auto (Compact)");
    state.update_size(size(100, 40));
    assert_eq!(state.settings_label(ToolOutputMode::Auto), "Auto (Full)");
    assert_eq!(ToolOutputMode::Auto.cycle(-1), ToolOutputMode::Full);
    assert_eq!(ToolOutputMode::Full.cycle(1), ToolOutputMode::Auto);
}

#[test]
fn folding_and_unfolding_never_changes_the_saved_message() {
    let mut app = AppState::new("thread".into());
    let idx = long_output(&mut app);
    let original = app.messages[idx].text.clone();
    assert!(screen(&mut app, 80).contains("source line 11"));
    app.tool_output.mode = ToolOutputMode::Compact;
    let compact = screen(&mut app, 80);
    assert!(compact.contains("→ Read main.rs"));
    assert!(compact.contains("12 rows hidden"));
    assert!(!compact.contains("source line 11"));
    assert_eq!(app.messages[idx].text, original);
    app.tool_output.mode = ToolOutputMode::Full;
    assert_eq!(screen(&mut app, 80), original);
}

#[test]
fn compact_hides_short_results_and_failure_details_but_keeps_answers_visible() {
    let mut app = AppState::new("thread".into());
    app.tool_output.mode = ToolOutputMode::Compact;
    app.append_message(Role::ToolOutput, "short output");
    let idx = long_output(&mut app);
    app.messages[idx]
        .tool_output
        .as_mut()
        .expect("details")
        .failed = true;
    app.append_message(Role::Assistant, "answer\n".repeat(10));
    let text = screen(&mut app, 80);
    assert!(!text.contains("short output"));
    assert!(!text.contains("source line 11"));
    assert!(text.contains("Failed:"));
    assert_eq!(text.matches("answer").count(), 10);
    assert!(text.contains("rows hidden"));
}

#[test]
fn compact_counts_wrapped_rows_and_keeps_the_cache_consistent_at_phone_widths() {
    for width in [8, 20, 40, 80] {
        let mut app = AppState::new("thread".into());
        app.tool_output.mode = ToolOutputMode::Compact;
        app.append_message(Role::ToolOutput, "let greeting = \"héllo 🌍\"; ".repeat(30));
        app.ensure_rendered_lines(width, None);
        let count = app.rendered_line_count();
        assert_eq!(app.snapshot_rendered_lines().len(), count);
        assert!(screen(&mut app, width).contains("Tool"));
    }
}

#[test]
fn a_folded_diff_keeps_filename_and_change_counts() {
    let mut app = AppState::new("thread".into());
    app.tool_output.mode = ToolOutputMode::Compact;
    app.append_diff_message(
        Role::ToolOutput,
        Some("main.rs".into()),
        "@@ -1,4 +1,4 @@\n-old1\n-old2\n-old3\n-old4\n+new1\n+new2\n+new3\n+new4\n",
    );
    assert_eq!(app.messages[0].kind, MessageKind::Diff);
    assert!(screen(&mut app, 80).contains("Δ Diff main.rs (+4, -4)"));
    app.tool_output.mode = ToolOutputMode::Full;
    assert!(screen(&mut app, 80).contains("+new4"));
}

#[test]
fn changing_modes_keeps_a_scrolled_message_in_view() {
    let mut app = AppState::new("thread".into());
    long_output(&mut app);
    app.append_message(Role::Assistant, "read this answer");
    app.ensure_rendered_lines(80, None);
    app.viewport.scroll_top = app.render_cache.rendered_block_offsets[1];
    app.viewport.auto_follow_bottom = false;
    app.tool_output.mode = ToolOutputMode::Compact;
    app.ensure_rendered_lines(80, None);
    assert_eq!(
        app.viewport.scroll_top,
        app.render_cache.rendered_block_offsets[1]
    );
    app.ensure_rendered_range_materialized(
        app.viewport.scroll_top + 1,
        app.viewport.scroll_top + 1,
    );
    assert_eq!(
        app.rendered_line_at(app.viewport.scroll_top + 1)
            .expect("answer")
            .text,
        "read this answer"
    );
}

#[test]
fn reload_and_fork_keep_the_preference_but_replace_message_contents() {
    let mut app = AppState::new("thread".into());
    app.tool_output.mode = ToolOutputMode::Compact;
    let mut replacement = AppState::new("thread".into());
    long_output(&mut replacement);
    app.replace_transcript_from(replacement);
    assert!(screen(&mut app, 80).contains("rows hidden"));
    app.reset_for_forked_thread("fork");
    assert_eq!(app.tool_output.mode, ToolOutputMode::Compact);
    assert!(app.messages.is_empty());
}

#[test]
fn preferences_save_only_the_explicit_choice_and_preserve_other_ui_fields() {
    let root = std::env::temp_dir().join(format!("carlos-tool-prefs-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).expect("create settings directory");
    let path = root.join("ui-settings.json");
    assert_eq!(load_mode_from(&path), ToolOutputMode::Auto);
    fs::write(&path, "{\"other\":true}").expect("write initial settings");
    for mode in [
        ToolOutputMode::Full,
        ToolOutputMode::Compact,
        ToolOutputMode::Auto,
    ] {
        save_mode_to(&path, mode).expect("save mode");
        assert_eq!(load_mode_from(&path), mode);
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("read settings"))
                .expect("parse settings");
        assert_eq!(value["other"], true);
        assert!(value.get("auto_compact").is_none());
    }
    fs::write(&path, "{\"tool_output\":\"unknown\"}").expect("write unknown mode");
    assert_eq!(load_mode_from(&path), ToolOutputMode::Auto);
    fs::write(&path, "broken").expect("write invalid JSON");
    assert_eq!(load_mode_from(&path), ToolOutputMode::Auto);
    fs::remove_file(path).expect("remove test settings");
    fs::remove_dir(root).expect("remove test directory");
}
