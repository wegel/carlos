//! Keep prompt choices exact through native binding, retired records and unknown-source history.

use carlos_proto::bytes::ByteOffset;
use carlos_proto::prompts::{PromptDirection, PromptSource};
use serde_json::json;

use crate::session::models::Role;
use crate::session::state::SessionState;

#[test]
fn selecting_a_transcript_row_resolves_its_exact_prompt_despite_identical_text() {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    let mut saved = Vec::new();
    for _ in 0..2 {
        state.record_prompt_history("same", None);
        let record = state
            .select_prompt(PromptSource::Human, None, PromptDirection::Latest)
            .expect("pending record")
            .record
            .expect("record");
        let position = state.append_message(Role::User, "same");
        let entry = state.messages()[position].id.to_wire();
        state.record_prompt_history("same", Some(state.messages()[position].id));
        saved.push((entry, record.id));
    }
    for (entry, prompt) in saved {
        let selected = state
            .select_prompt(PromptSource::Human, Some(&entry), PromptDirection::AtEntry)
            .expect("exact visible row")
            .record
            .expect("bound record");
        assert_eq!(selected.id, prompt);
        assert_eq!(selected.entry, Some(entry.clone()));
        assert!(state
            .select_prompt(
                PromptSource::Unknown,
                Some(&entry),
                PromptDirection::AtEntry
            )
            .is_err());
    }
    assert!(state
        .select_prompt(PromptSource::Human, None, PromptDirection::AtEntry)
        .is_err());
    let foreign = format!("{}-0000000000000001", "f".repeat(32))
        .parse()
        .expect("foreign entry");
    assert!(state
        .select_prompt(
            PromptSource::Human,
            Some(&foreign),
            PromptDirection::AtEntry
        )
        .is_err());
}

#[test]
fn a_pending_record_keeps_its_identity_when_bound_and_a_retired_id_cannot_choose_equal_text() {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    state.record_prompt_history("same", None);
    let selection = state
        .select_prompt(PromptSource::Human, None, PromptDirection::Latest)
        .expect("pending prompt");
    let pending = selection.record.expect("unbound record");
    assert!(pending.entry.is_none());
    assert_eq!(
        state
            .prompt_text(&pending.id, ByteOffset::default(), 4)
            .expect("original")
            .text,
        "same"
    );
    assert!(state.discard_pending_prompt("same"));
    state.record_prompt_history("same", None);
    assert!(state
        .prompt_text(&pending.id, ByteOffset::default(), 4)
        .is_err());
    let live = state
        .select_prompt(PromptSource::Human, None, PromptDirection::Latest)
        .expect("replacement")
        .record
        .expect("record");
    assert_ne!(live.id, pending.id);
    let index = state.append_message(Role::User, "same");
    let entry = state.messages()[index].id;
    state.record_prompt_history("same", Some(entry));
    let bound = state
        .select_prompt(PromptSource::Human, None, PromptDirection::Latest)
        .expect("bound")
        .record
        .expect("record");
    assert_eq!(bound.id, live.id);
    assert_eq!(bound.entry, Some(entry.to_wire()));
}

#[test]
fn unknown_sources_are_explicit_and_automatic_prompts_never_enter_human_recall() {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    for (source, text) in [
        ("human", "human 日本語"),
        ("unknown", "literal unknown"),
        ("automatic", "automatic"),
    ] {
        let key = state.backend_entry_key(source);
        state.append_claude_history_prompt(
            &json!({"source":source,"text":text,"uuid":source}),
            &key,
        );
    }
    let human = state
        .select_prompt(PromptSource::Human, None, PromptDirection::Latest)
        .expect("human only");
    assert!(!human.has_older);
    let human = human.record.expect("human record");
    let unknown = state
        .select_prompt(PromptSource::Unknown, None, PromptDirection::Latest)
        .expect("explicit unknown")
        .record
        .expect("unknown record");
    assert_ne!(human.id, unknown.id);
    assert!(state
        .select_prompt(
            PromptSource::Human,
            Some(&unknown.id),
            PromptDirection::Older
        )
        .is_err());
    assert_eq!(
        state
            .prompt_text(&unknown.id, ByteOffset::default(), 65536)
            .expect("original unknown")
            .text,
        "literal unknown"
    );
}

#[test]
fn utf8_chunks_preserve_all_bytes_and_reject_interior_or_past_end_offsets() {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    let original = "🌍日本語\n".repeat(12000);
    state.record_prompt_history(&original, None);
    let prompt = state
        .select_prompt(PromptSource::Human, None, PromptDirection::Latest)
        .expect("large prompt")
        .record
        .expect("record");
    let mut offset = ByteOffset::default();
    let mut copied = String::new();
    loop {
        let chunk = state
            .prompt_text(&prompt.id, offset, 65536)
            .expect("bounded chunk");
        assert!(chunk.text.len() <= 65536);
        copied.push_str(&chunk.text);
        let Some(next) = chunk.next else { break };
        offset = next;
    }
    assert_eq!(copied, original);
    assert!(state
        .prompt_text(&prompt.id, ByteOffset::new(1), 4)
        .is_err());
    assert!(state
        .prompt_text(&prompt.id, ByteOffset::new(original.len() as u64 + 1), 4)
        .is_err());
    let end = state
        .prompt_text(&prompt.id, ByteOffset::new(original.len() as u64), 4)
        .expect("exact EOF");
    assert!(end.text.is_empty());
    assert!(end.next.is_none());
}
