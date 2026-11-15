//! Keep duplicate preparation and initial-startup guards separate from private keyboard checks.

use serde_json::json;

use super::{begin, core};
use crate::backend::{BackendKind, PromptOrigin};
use crate::session::claude::ClaudePhase;
use crate::session::entry_id::EntryId;
use crate::session::models::Role;
use crate::session::prompt_commands::PromptContext;
use crate::session::state::SessionState;

fn anchor(core: &mut SessionState) -> EntryId {
    let index = core.append_message(Role::User, "old prompt");
    core.record_prompt_history("old prompt", Some(core.messages()[index].id));
    core.input_history()
        .prompt_id(0)
        .expect("real selected prompt")
}

#[test]
fn preparing_edit_ignores_duplicate_enter_and_only_the_matching_accepted_prompt_finishes_it() {
    let mut core = core();
    let selected = anchor(&mut core);
    begin(&mut core, "saved edit");
    for _ in 0..3 {
        let duplicate = core
            .prepare_rewind_text(
                BackendKind::Claude,
                selected,
                "newer composer".into(),
                &PromptContext {
                    operation: None,
                    claude_queue: false,
                },
                false,
            )
            .expect("phase guard");
        assert!(duplicate.work.is_none());
    }
    assert_eq!(core.claude_edit_prompt_for_test(), Some("saved edit"));
    core.queue_claude_edit("saved edit");
    core.dispatch_claude_edit("saved edit", PromptOrigin::Human, None);
    core.receive_claude_edit_acceptance("different-uuid", &mut Default::default());
    assert!(core.claude_edit_progress().is_some());
    core.bind_claude_edit(
        "saved edit",
        PromptOrigin::Human,
        None,
        &json!({"turn":{"id":"exact-sdk-uuid"}}),
    );
    core.receive_claude_edit_acceptance("different-uuid", &mut Default::default());
    assert!(core.claude_edit_progress().is_some());
    core.receive_claude_edit_acceptance("exact-sdk-uuid", &mut Default::default());
    assert!(core.claude_edit_progress().is_none());
}

#[test]
fn enter_during_initial_startup_does_not_reopen_the_helper_or_submit_an_edit() {
    let mut core = core();
    let selected = anchor(&mut core);
    core.set_claude_phase_for_test(ClaudePhase::Starting);
    let prepared = core
        .prepare_rewind_text(
            BackendKind::Claude,
            selected,
            "edit while starting".into(),
            &PromptContext {
                operation: None,
                claude_queue: false,
            },
            false,
        )
        .expect("native ready guard");
    assert!(prepared.work.is_none());
    assert_eq!(core.claude().phase(), ClaudePhase::Starting);
    assert!(core.claude_edit_prompt_for_test().is_none());
}
