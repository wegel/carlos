//! Enumerate prompt navigation and rewind rules against fixed pre-split results.

use carlos_core::session::models::Role;
use serde_json::json;

use crate::app::item_history::load_history_from_start_or_resume;
use crate::app::state::AppState;

#[path = "input_history_split_fork_tests.rs"]
mod fork_tests;

fn seed() -> AppState {
    let mut app = AppState::new("root".into());
    for text in ["first", "second"] {
        let index = app.append_message(Role::User, text);
        app.record_input_history(text, Some(index));
    }
    app.set_input_text("draft 日本語");
    app
}

#[test]
fn navigation_clamps_at_oldest_and_restores_the_first_draft_at_newest() {
    let mut app = seed();
    for (up, changed, text, selected) in [
        (false, false, "draft 日本語", None),
        (true, true, "second", Some(1)),
        (true, true, "first", Some(0)),
        (true, true, "first", Some(0)),
        (false, true, "second", Some(1)),
        (false, true, "draft 日本語", None),
        (false, false, "draft 日本語", None),
    ] {
        let result = if up {
            app.navigate_input_history_up()
        } else {
            app.navigate_input_history_down()
        };
        assert_eq!(result, changed);
        assert_eq!(app.input_text(), text);
        assert_eq!(app.rewind_selected_message_idx(), selected);
    }
    let mut empty = AppState::new("empty".into());
    empty.set_input_text("unsent");
    assert!(!empty.navigate_input_history_up());
    assert!(!empty.navigate_input_history_down());
    assert_eq!(empty.input_text(), "unsent");
}

#[test]
fn rewind_restores_its_original_draft_and_clear_keeps_the_existing_cursor() {
    for clear in [false, true] {
        let mut app = seed();
        app.enter_rewind_mode();
        assert_eq!(app.input_text(), "second");
        app.set_input_text("edited second");
        app.enter_rewind_mode();
        assert_eq!(app.input_text(), "edited second");
        assert_eq!(app.rewind_selected_message_idx(), Some(1));
        if clear {
            app.clear_rewind_mode_state();
            assert_eq!(app.rewind_selected_message_idx(), Some(1));
            assert!(app.navigate_input_history_up());
            assert_eq!(app.input_text(), "first");
        } else {
            app.exit_rewind_mode_restore();
            assert_eq!(app.input_text(), "draft 日本語");
            assert_eq!(app.rewind_selected_message_idx(), None);
            assert!(app.navigate_input_history_up());
            assert_eq!(app.input_text(), "second");
        }
        assert!(!app.rewind_mode());
        app.exit_rewind_mode_restore();
        assert!(app.navigate_input_history_down());
        assert_eq!(
            app.input_text(),
            if clear { "second" } else { "draft 日本語" }
        );
    }
}

#[test]
fn recording_and_discarding_reset_the_cursor_only_for_the_original_cases() {
    for (operation, selected, length) in [
        ("duplicate", Some(1), 2),
        ("empty", None, 2),
        ("append", None, 3),
        ("discard-bound", Some(1), 2),
        ("discard-mismatch", Some(1), 2),
    ] {
        let mut app = seed();
        app.enter_rewind_mode();
        match operation {
            "duplicate" => app.record_input_history("second", Some(1)),
            "empty" => app.record_input_history("", None),
            "append" => app.push_input_history("pending"),
            "discard-bound" => app.discard_pending_input_history("second"),
            "discard-mismatch" => app.discard_pending_input_history("different"),
            _ => unreachable!(),
        }
        assert_eq!(app.rewind_selected_message_idx(), selected, "{operation}");
        assert_eq!(app.input_history_len(), length, "{operation}");
        assert!(app.rewind_mode());
    }
    let mut app = seed();
    app.push_input_history("pending");
    app.enter_rewind_mode();
    assert_eq!(app.rewind_selected_message_idx(), None);
    let index = app.append_message(Role::User, "pending");
    app.record_input_history("pending", Some(index));
    assert_eq!(app.input_history_len(), 3);
    assert_eq!(app.rewind_selected_history_index(), None);
    app.push_input_history("unsent");
    app.navigate_input_history_up();
    app.discard_pending_input_history("unsent");
    assert_eq!(app.rewind_selected_history_index(), None);
    assert_eq!(app.input_history_len(), 3);
}

#[test]
fn replacing_history_during_rewind_resets_focus_but_preserves_the_composer() {
    for text in ["", "new prompt"] {
        let mut app = seed();
        app.enter_rewind_mode();
        app.set_input_text("edit remains");
        let response = json!({"result":{"thread":{"id":"root","turns":[
            {"id":"new","status":"completed","items":[
                {"type":"userMessage","id":"new-user","content":[{"type":"text","text":text}]}
            ]}
        ]}}});
        load_history_from_start_or_resume(&mut app, &response.to_string()).expect("replace");
        assert!(!app.rewind_mode());
        assert_eq!(app.rewind_selected_history_index(), None);
        assert_eq!(app.input_text(), "edit remains");
        assert_eq!(app.navigate_input_history_up(), !text.is_empty());
        if !text.is_empty() {
            assert_eq!(app.input_text(), "new prompt");
            assert!(app.navigate_input_history_down());
            assert_eq!(app.input_text(), "edit remains");
        }
    }
}
