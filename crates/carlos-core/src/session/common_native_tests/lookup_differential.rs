//! Compare id-based lookups to the former index-map algorithm over generated edit sequences.

use crate::backend::BackendKind;
use crate::session::models::EntryDraft;
use crate::session::models::MessageKind;
use crate::session::models::Role;
use serde_json::json;

use super::head_reference::HeadReference as IndexReference;
use super::identity_support::{assert_entries, session_for as app, truncate};
use crate::session::item_history::load_history_from_start_or_resume;
use crate::session::state::SessionState;

fn old_key(turn: &str, item: &str, part: usize) -> String {
    format!("codex:{}", json!([turn, item, part]))
}

fn reference_upsert(old: &mut IndexReference, key: String, row: EntryDraft) {
    old.upsert_mapped_message(&key, row.role, row.text, row.kind, row.file_path);
}

fn check(
    app: &SessionState,
    old: &IndexReference,
    seed: u64,
    step: usize,
    absent: Option<(&str, &str, usize)>,
) {
    assert_entries(app);
    assert_eq!(
        app.messages().len(),
        old.messages.len(),
        "seed {seed} step {step}"
    );
    for (message, row) in app.messages().iter().zip(&old.messages) {
        assert_eq!(
            (
                message.role,
                message.kind,
                &message.text,
                &message.file_path,
                message
                    .tool_output
                    .as_ref()
                    .map(|t| (&t.summary, t.generated, t.failed))
            ),
            (
                row.role,
                row.kind,
                &row.text,
                &row.file_path,
                row.tool_output
                    .as_ref()
                    .map(|t| (&t.summary, t.generated, t.failed))
            ),
            "seed {seed} step {step}"
        );
        assert_eq!(
            app.messages().entry(message.id).expect("live id").id,
            message.id
        );
    }
    check_keys(app, old, seed, step, absent);
}

fn check_keys(
    app: &SessionState,
    old: &IndexReference,
    seed: u64,
    step: usize,
    absent: Option<(&str, &str, usize)>,
) {
    for turn in ["t0", "t1", "t2"] {
        for item in ["a", "b", "x#part-1"] {
            for part in 0..3 {
                let key = crate::session::entry_key::BackendEntryKey {
                    backend: BackendKind::Codex,
                    conversation: "thread".into(),
                    turn: Some(turn.into()),
                    item: crate::session::entry_key::NativeEntryKey::Item(item.into()),
                    part,
                };
                let actual = app
                    .messages()
                    .entry_id_for_item(&key)
                    .and_then(|id| app.messages().position(id));
                assert_eq!(
                    actual,
                    if absent.is_some_and(|(t, i, count)| t == turn && i == item && part >= count) {
                        // Named exception: Entry identity rules retire absent parts;
                        // unchanged HEAD deliberately retains these old lookups.
                        None
                    } else {
                        old.item_to_message_index
                            .get(&old_key(turn, item, part))
                            .copied()
                    },
                    "seed {seed} step {step}: {key:?}"
                );
            }
        }
        assert_eq!(
            app.messages()
                .turn_diff(turn)
                .and_then(|id| app.messages().position(id)),
            old.turn_diff_to_index.get(turn).copied()
        );
    }
}

fn change_parts(
    app: &mut SessionState,
    old: &mut IndexReference,
    turn: &str,
    item: &str,
    count: usize,
) {
    app.transcript_retain_parts(&app.backend_entry_key(item), count);
    for part in 0..count {
        let row = EntryDraft {
            kind: MessageKind::Diff,
            file_path: Some(format!("{part}.rs")),
            ..EntryDraft::plain(Role::ToolOutput, format!("+part {part}"))
        };
        app.with_native_item_part(item, part, |app, id| {
            app.upsert_mapped_message(
                &app.backend_entry_key(id),
                row.role,
                row.text.clone(),
                row.kind,
                row.file_path.clone(),
            );
        });
        reference_upsert(old, old_key(turn, item, part), row);
    }
}

fn replace(app: &mut SessionState, old: &mut IndexReference, turn: &str, discovered: bool) {
    let mut items = vec![json!({"type":"agentMessage","id":"a","text":"corrected"})];
    if discovered {
        items.insert(0, json!({"type":"agentMessage","id":"b","text":"inserted"}));
    }
    let response = json!({"result":{"thread":{"id":"thread","turns":[{"id":turn,"items":items}]}}});
    load_history_from_start_or_resume(app, &response.to_string()).expect("checked history");
    *old = IndexReference::default();
    if discovered {
        reference_upsert(
            old,
            old_key(turn, "b", 0),
            EntryDraft::plain(Role::Assistant, "inserted"),
        );
    }
    reference_upsert(
        old,
        old_key(turn, "a", 0),
        EntryDraft::plain(Role::Assistant, "corrected"),
    );
}

fn aggregate(app: &mut SessionState, old: &mut IndexReference, turn: &str) {
    app.upsert_turn_diff(turn, "+aggregate");
    old.upsert_turn_diff(turn, "+aggregate");
}

fn complete_or_read(
    app: &mut SessionState,
    old: &mut IndexReference,
    bits: u64,
    item: &str,
    key: String,
) {
    let row = EntryDraft::plain(
        if bits % 9 == 2 {
            Role::Assistant
        } else {
            Role::ToolCall
        },
        if bits % 9 == 2 {
            "completed"
        } else {
            "→ Read a.rs"
        },
    );
    let row = if bits % 9 == 3 {
        EntryDraft {
            file_path: Some("a.rs".into()),
            ..row
        }
    } else {
        row
    };
    app.upsert_mapped_message(
        &app.backend_entry_key(item),
        row.role,
        row.text.clone(),
        row.kind,
        row.file_path.clone(),
    );
    reference_upsert(old, key.clone(), row);
    if bits % 9 == 2 {
        app.mark_item_completed(&app.backend_entry_key(item));
        old.mark_item_completed(&key);
    }
}

fn apply(
    app: &mut SessionState,
    old: &mut IndexReference,
    bits: u64,
) -> Option<(&'static str, &'static str, usize)> {
    let turn = ["t0", "t1", "t2"][(bits as usize / 11) % 3];
    let item = ["a", "b", "x#part-1"][(bits as usize / 37) % 3];
    app.replace_codex_item_turn(Some(turn.into()));
    let key = old_key(turn, item, 0);
    match bits % 9 {
        0 => {
            app.ensure_item_placeholder(&app.backend_entry_key(item), Role::Assistant);
            if !old.item_to_message_index.contains_key(&key) {
                reference_upsert(old, key, EntryDraft::plain(Role::Assistant, ""));
            }
        }
        1 => {
            app.upsert_agent_delta(&app.backend_entry_key(item), " delta");
            old.upsert_agent_delta(&key, " delta");
        }
        2 | 3 => {
            complete_or_read(app, old, bits, item, key);
        }
        4 => {
            let count = (bits as usize / 97) % 3 + 1;
            let absent = (count..3).any(|part| {
                old.item_to_message_index
                    .contains_key(&old_key(turn, item, part))
            });
            change_parts(app, old, turn, item, count);
            return absent.then_some((turn, item, count));
        }
        5 => {
            let cut = bits as usize / 97 % (old.messages.len() + 1);
            truncate(app, cut);
            old.truncate(cut);
        }
        6 => replace(app, old, turn, bits & 16 != 0),
        7 => {
            aggregate(app, old, turn);
        }
        _ => {
            app.append_message(Role::System, "local notice");
            old.messages
                .push(EntryDraft::plain(Role::System, "local notice"));
        }
    }
    None
}

#[test]
fn generated_sequences_resolve_the_same_entries_as_the_old_index_maps() {
    for seed in 1..=24 {
        let mut bits = seed;
        let mut app = app(BackendKind::Codex);
        let mut old = IndexReference::default();
        for step in 0..240 {
            bits ^= bits << 13;
            bits ^= bits >> 7;
            bits ^= bits << 17;
            let absent = apply(&mut app, &mut old, bits);
            check(&app, &old, seed, step, absent);
            if absent.is_some() {
                // End this trace at the documented exception. A separate test
                // checks regrowth, whose new row intentionally differs from HEAD.
                app = super::identity_support::session_for(BackendKind::Codex);
                old = IndexReference::default();
            }
        }
    }
}
