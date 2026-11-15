//! Preserve fragment recursion, source defaults and failed-start row identity in Core.

use carlos_proto::submissions::{RecoveryKind, SubmissionOutcome};
use serde_json::json;

use super::{control, feed, named, new_session};
use crate::session::backend_update::BackendChange;
use crate::session::claude::receive_update::ClaudeEffect;
use crate::session::claude::ClaudePhase;
use crate::session::models::{MessageKind, Role};

#[test]
fn normalized_parts_reject_bounds_without_erasing_earlier_rows() {
    for params in [
        json!({"offset":1,"totalBytes":2,"text":"x"}),
        json!({"offset":0,"totalBytes":67108865,"text":"x"}),
        json!({"offset":0,"totalBytes":1,"text":"xx"}),
        json!({"totalBytes":2,"text":"x"}),
        json!({"offset":0,"text":"x"}),
        json!({"offset":0,"totalBytes":2}),
        json!({"offset":0,"totalBytes":1,"text":"!"}),
    ] {
        let mut core = new_session("session");
        core.append_message(Role::Assistant, "earlier");
        let mut params = params;
        params["generation"] = json!(1);
        params["epoch"] = json!(0);
        assert!(named(&mut core, "claude/historyFactPart", &params).is_none());
        assert_eq!(core.claude().phase(), ClaudePhase::Closed);
        assert_eq!(core.messages()[0].text, "earlier");
        assert!(core.messages().len() >= 2);
    }
}

#[test]
fn normalized_completion_routes_again_and_acknowledges_even_when_inner_fact_is_dropped() {
    for epoch in [0, 4] {
        let mut core = new_session("session");
        named(
            &mut core,
            "claude/conversationReset",
            &json!({"generation":1,"epoch":4,"new_conversation_id":"reset"}),
        );
        let text = json!({"method":"claude/historyPrompt","params":{
            "generation":999,"epoch":epoch,"source":"human","uuid":"prompt","text":"row"}})
        .to_string();
        let split = text.len() / 2;
        for (offset, piece) in [(0, &text[..split]), (split, &text[split..])] {
            let params = json!({"generation":1,"epoch":4,"offset":offset,"totalBytes":text.len(),"text":piece});
            control(
                named(&mut core, "claude/historyFactPart", &params),
                "history.continue",
                json!({"generation":1,"epoch":4}),
            );
        }
        assert_eq!(core.messages().len(), usize::from(epoch == 4));
        if epoch == 4 {
            assert_eq!(core.messages()[0].text, "row");
        }
    }
}

#[test]
fn source_defaults_duplicates_and_empty_uuids_keep_their_old_rows() {
    let mut core = new_session("session");
    core.enable_claude();
    for facts in [
        json!({"uuid":"same","source":"human","text":"human","inputKind":"steer"}),
        json!({"uuid":"same","source":"automatic","text":"duplicate"}),
        json!({"uuid":"legacy","text":"old"}),
        json!({"source":"automatic","text":"automatic"}),
        json!({"source":"system","uuid":"system","text":"hidden"}),
        json!({}),
    ] {
        core.append_claude_history_prompt(
            &facts,
            &core.backend_entry_key(&format!(
                "claude-prompt:{}",
                facts["uuid"].as_str().unwrap_or("")
            )),
        );
    }
    let rows: Vec<_> = core.messages().iter().map(|m| m.text.as_str()).collect();
    assert_eq!(
        rows,
        [
            "human",
            "[Source unknown] old",
            "Automatic prompt: automatic",
            "[Source unknown] "
        ]
    );
    assert_eq!(core.messages()[0].kind, MessageKind::UserSteer);
    assert_eq!(core.input_history().history_len(), 1);
    assert_eq!(core.claude_history().unknown().len(), 2);
}

#[test]
fn failed_start_after_switch_keeps_the_adopted_session_and_its_rows() {
    let mut core = new_session("original");
    core.begin_claude_edit("recover".into());
    named(
        &mut core,
        "claude/sessionChanged",
        &json!({"generation":2,"sessionId":"branch"}),
    );
    core.append_message(Role::Assistant, "branch row");
    let id = core.messages()[0].id;
    let update = feed(
        &mut core,
        "claude/sessionStartFailed",
        &json!({"generation":2}),
    );
    let BackendChange::Claude(update) = update.change else {
        panic!("native failure update");
    };
    assert!(update.effects.iter().any(|effect| matches!(effect,
        ClaudeEffect::Submission(SubmissionOutcome::Recovered { draft, kind: RecoveryKind::Edit }) if draft.text == "recover"
    )));
    assert_eq!(core.thread_id(), "branch");
    assert_eq!(core.messages()[0].id, id);
    assert_eq!(core.messages()[0].text, "branch row");
    assert_eq!(core.claude().phase(), ClaudePhase::Closed);
}
