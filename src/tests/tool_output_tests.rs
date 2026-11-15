//! Check tool modes, reversible folding, scroll anchors, and saved preferences.

use std::fs;

use carlos_core::session::models::MessageKind;
use carlos_core::session::models::Role;
use carlos_core::session::models::ToolOutputDetails;

use super::{render_fixture, AppState, TerminalSize};
use crate::app::perf_view::PerfView;
use crate::app::state::ModelSettingsField;
use crate::app::tool_output_preferences::{load_mode_from, save_mode_to};
use crate::app::tool_output_state::{ToolOutputMode, ToolOutputState};
use carlos_core::perf_fixture::PerfRow;
use carlos_core::session::models::EntryDraft;

fn size(width: usize, height: usize) -> TerminalSize {
    TerminalSize { width, height }
}

fn long_output(app: &mut AppState) -> usize {
    let text = (0..12)
        .map(|i| format!("source line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let idx = app.append_message(Role::ToolOutput, text);
    app.session.transcript_set_tool(
        idx,
        ToolOutputDetails {
            generated: false,
            summary: "→ Read main.rs".into(),
            failed: false,
        },
    );
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

fn plain_output() -> PerfRow {
    render_fixture::draft(EntryDraft {
        role: Role::ToolOutput,
        kind: MessageKind::Plain,
        text: (0..12)
            .map(|i| format!("source line {i}"))
            .collect::<Vec<_>>()
            .join("\n"),
        file_path: None,
        tool_output: Some(ToolOutputDetails {
            generated: false,
            summary: "→ Read main.rs".into(),
            failed: false,
        }),
    })
}

fn plain_screen(view: &mut PerfView, width: usize) -> String {
    render_fixture::snapshot(view, width)
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
    app.view.tool_output.mode = ToolOutputMode::Full;
    app.open_model_settings();
    app.model_settings_move_field(false);
    assert_eq!(
        app.view.runtime_settings.model_settings_field,
        ModelSettingsField::ToolOutput
    );
    assert_eq!(
        app.view.runtime_settings.model_settings_tool_output,
        ToolOutputMode::Full
    );
    app.view.runtime_settings.model_settings_tool_output = ToolOutputMode::Compact;
    app.close_model_settings();
    assert_eq!(app.view.tool_output.mode, ToolOutputMode::Full);
    app.open_model_settings();
    assert_eq!(
        app.view.runtime_settings.model_settings_tool_output,
        ToolOutputMode::Full
    );
    app.view.runtime_settings.model_settings_tool_output = ToolOutputMode::Compact;
    app.apply_model_settings();
    assert_eq!(app.view.tool_output.mode, ToolOutputMode::Compact);
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
    let mut view = render_fixture::view(vec![plain_output()]);
    let original = view.rows[0].text().to_owned();
    assert!(plain_screen(&mut view, 80).contains("source line 11"));
    view.view.tool_output.mode = ToolOutputMode::Compact;
    let compact = plain_screen(&mut view, 80);
    assert!(compact.contains("main.rs"));
    assert!(compact.trim_end().ends_with("12"));
    assert!(!compact.contains("source line 11"));
    assert_eq!(view.rows[0].text(), original);
    view.view.tool_output.mode = ToolOutputMode::Full;
    assert_eq!(plain_screen(&mut view, 80), original);
}

#[test]
fn compact_hides_short_results_and_failure_details_but_keeps_answers_visible() {
    let mut failed = plain_output();
    failed.entry.tool.as_mut().expect("details").status = carlos_proto::session::ToolStatus::Failed;
    let mut view = render_fixture::view(vec![
        render_fixture::row(Role::ToolOutput, "short output"),
        failed,
        render_fixture::row(Role::Assistant, "answer\n".repeat(10)),
    ]);
    view.view.tool_output.mode = ToolOutputMode::Compact;
    let text = plain_screen(&mut view, 80);
    assert!(!text.contains("short output"));
    assert!(!text.contains("source line 11"));
    assert!(text.contains("failed"));
    assert_eq!(text.matches("answer").count(), 10);
    assert!(text.lines().any(|line| line.trim_end().ends_with("12")));
}

#[test]
fn compact_counts_wrapped_rows_and_keeps_the_cache_consistent_at_phone_widths() {
    for width in [8, 20, 40, 80] {
        let mut view = render_fixture::view(vec![render_fixture::row(
            Role::ToolOutput,
            "let greeting = \"héllo 🌍\"; ".repeat(30),
        )]);
        view.view.tool_output.mode = ToolOutputMode::Compact;
        view.ensure_rendered_lines(width);
        let count = view.rendered_line_count();
        let lines = render_fixture::snapshot_current(&mut view);
        assert_eq!(lines.len(), count);
        assert!(lines.iter().all(|line| line.cells <= width));
    }
}

#[test]
fn a_folded_diff_keeps_filename_and_change_counts() {
    let mut view = render_fixture::view(vec![render_fixture::draft(EntryDraft {
        role: Role::ToolOutput,
        kind: MessageKind::Diff,
        file_path: Some("main.rs".into()),
        text: "@@ -1,4 +1,4 @@\n-old1\n-old2\n-old3\n-old4\n+new1\n+new2\n+new3\n+new4\n".into(),
        tool_output: None,
    })]);
    view.view.tool_output.mode = ToolOutputMode::Compact;
    assert_eq!(
        view.rows[0].entry.format,
        carlos_proto::session::EntryFormat::Diff
    );
    assert!(plain_screen(&mut view, 80).contains("main.rs +4 −4"));
    view.view.tool_output.mode = ToolOutputMode::Full;
    assert!(plain_screen(&mut view, 80).contains("+new4"));
}

#[test]
fn reload_and_fork_keep_the_preference_but_replace_message_contents() {
    let mut app = AppState::new("thread".into());
    app.view.tool_output.mode = ToolOutputMode::Compact;
    let mut replacement = AppState::new("thread".into());
    long_output(&mut replacement);
    app.replace_transcript_for_test(replacement);
    assert!(screen(&mut app, 80).trim_end().ends_with("12"));
    app.reset_for_forked_thread("fork");
    assert_eq!(app.view.tool_output.mode, ToolOutputMode::Compact);
    assert!(app.session.messages().is_empty());
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
    fs::remove_dir_all(root).expect("remove private settings directory and retained lock");
}
