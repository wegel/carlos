//! Exhaustive small histories exercise ids independently of app widgets.

use std::collections::HashSet;

use super::Transcript;
use crate::backend::BackendKind;
use crate::session::entry_key::{BackendEntryKey, NativeEntryKey};
use crate::session::history_identity::{HistoryEntry, HistoryMode};
use crate::session::models::{EntryDraft, Role};

fn key(backend: BackendKind, item: &str, part: usize) -> BackendEntryKey {
    BackendEntryKey {
        backend,
        conversation: "conversation".into(),
        turn: Some("turn".into()),
        item: if backend == BackendKind::Claude {
            NativeEntryKey::ClaudeAssistant {
                id: item.into(),
                block: 0,
            }
        } else {
            NativeEntryKey::Item(item.into())
        },
        part,
    }
}

fn draft(key: Option<BackendEntryKey>, text: &str) -> HistoryEntry {
    HistoryEntry {
        occurrences: 1,
        key,
        draft: EntryDraft::plain(Role::Assistant, text),
    }
}

fn backends() -> [BackendKind; 3] {
    [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok]
}

#[test]
fn same_thread_reload_preserves_ids_after_insertion() {
    for backend in backends() {
        for insertion in 0..=2 {
            let mut store = Transcript::new();
            let a = key(backend, "a", 0);
            let b = key(backend, "b", 0);
            let old = store.replace_history(
                vec![draft(Some(a.clone()), "a"), draft(Some(b.clone()), "b")],
                HistoryMode::SameThread,
            );
            let mut rows = vec![
                draft(Some(a.clone()), "a"),
                draft(Some(b.clone()), "corrected b"),
            ];
            rows.insert(insertion, draft(Some(key(backend, "x", 0)), "x"));
            let new = store.replace_history(rows, HistoryMode::SameThread);
            assert_eq!(store.entry_id_for_item(&a), Some(old[0]));
            assert_eq!(store.entry_id_for_item(&b), Some(old[1]));
            assert_eq!(store.entry(old[1]).expect("b retained").text, "corrected b");
            assert!(!old.contains(&new[insertion]));
            assert_eq!(store.position(old[0]), Some(usize::from(insertion == 0)));
        }
    }
}

#[test]
fn missing_ambiguous_and_generated_keys_never_match_prose_or_positions() {
    for backend in backends() {
        for old_count in 0..=3 {
            for new_count in 0..=3 {
                let native = key(backend, "duplicate", 0);
                let mut store = Transcript::new();
                let old = store.replace_history(
                    (0..old_count)
                        .map(|_| draft(Some(native.clone()), "same"))
                        .collect(),
                    HistoryMode::SameThread,
                );
                let new = store.replace_history(
                    (0..new_count)
                        .map(|_| draft(Some(native.clone()), "same"))
                        .collect(),
                    HistoryMode::SameThread,
                );
                if old_count == 1 && new_count == 1 {
                    assert_eq!(old, new);
                } else {
                    assert!(old.iter().all(|id| !new.contains(id)));
                }
                assert_eq!(store.entry_id_for_item(&native).is_some(), new_count == 1);
                for id in old.iter().filter(|id| !new.contains(id)) {
                    assert!(store.entry(*id).is_none());
                }
            }
        }
    }
    for source in [
        None,
        Some(BackendEntryKey {
            item: NativeEntryKey::ClaudeTransient("claude-msg-0-1-0".into()),
            ..key(BackendKind::Claude, "unused", 0)
        }),
    ] {
        let mut store = Transcript::new();
        let old = store.replace_history(
            vec![draft(source.clone(), "identical")],
            HistoryMode::SameThread,
        );
        let new = store.replace_history(vec![draft(source, "identical")], HistoryMode::SameThread);
        assert_ne!(
            old, new,
            "Claude counters and id-less records intentionally remint"
        );
    }
}

#[test]
fn fork_remints_prefix_and_retires_old_ids() {
    for backend in backends() {
        for length in 0..5 {
            let mut store = Transcript::new();
            let rows = || {
                (0..length)
                    .map(|index| draft(Some(key(backend, &index.to_string(), 0)), "copy"))
                    .collect()
            };
            let old = store.replace_history(rows(), HistoryMode::SameThread);
            let new = store.replace_history(rows(), HistoryMode::NewConversation);
            assert!(old
                .iter()
                .all(|id| !new.contains(id) && store.entry(*id).is_none()));
            for (index, id) in new.iter().enumerate() {
                assert_eq!(store.position(*id), Some(index));
            }
        }
    }
}

#[test]
fn truncate_never_reuses_a_removed_id() {
    for length in 1..8 {
        for cut in 0..length {
            let mut store = Transcript::new();
            let old: Vec<_> = (0..length)
                .map(|index| {
                    let id = store.append(EntryDraft::plain(Role::Assistant, "text"));
                    store.bind_item(key(BackendKind::Codex, &index.to_string(), 0), id);
                    store.bind_turn_diff(index.to_string(), id);
                    id
                })
                .collect();
            assert!(store.truncate_before(old[cut]));
            for (index, id) in old.iter().enumerate() {
                assert_eq!(store.entry(*id).is_some(), index < cut);
                assert_eq!(
                    store
                        .entry_id_for_item(&key(BackendKind::Codex, &index.to_string(), 0))
                        .is_some(),
                    index < cut
                );
                assert_eq!(store.turn_diff(&index.to_string()).is_some(), index < cut);
            }
            for index in cut..length {
                let id = store.append(EntryDraft::plain(Role::Assistant, "reintroduced"));
                store.bind_item(key(BackendKind::Codex, &index.to_string(), 0), id);
                assert!(!old.contains(&id));
            }
            assert!(!store.truncate_before(old[cut]));
        }
    }
}

#[test]
fn cards_rows_resets_and_namespaces_never_reuse_ids() {
    let mut seen = HashSet::new();
    let mut store = Transcript::new();
    for _ in 0..100 {
        assert!(seen.insert(store.allocate_card()));
        assert!(seen.insert(store.append(EntryDraft::plain(Role::System, "notice"))));
        store.reset();
    }
}

#[test]
fn typed_parts_backend_conversation_and_turn_keys_do_not_alias() {
    let mut store = Transcript::new();
    let mut keys = Vec::new();
    for backend in backends() {
        for conversation in ["a", "b"] {
            for turn in ["a", "b"] {
                for item in ["x", "x#part-1"] {
                    for part in 0..3 {
                        let mut native = key(backend, item, part);
                        native.conversation = conversation.into();
                        native.turn = Some(turn.into());
                        let id = store.append(EntryDraft::plain(Role::ToolOutput, "diff"));
                        store.bind_item(native.clone(), id);
                        keys.push((native, id));
                    }
                }
            }
        }
    }
    assert_eq!(keys.len(), store.items().len());
    for (key, id) in keys {
        assert_eq!(store.entry_id_for_item(&key), Some(id));
    }
}

#[test]
fn unkeyed_adoption_retires_native_maps_without_reusing_card_or_row_ids() {
    for mode in [HistoryMode::SameThread, HistoryMode::NewConversation] {
        for count in 0..5 {
            let mut store = Transcript::new();
            let native = key(BackendKind::Codex, "old", 0);
            let old = store.append(EntryDraft::plain(Role::User, "same"));
            store.bind_item(native.clone(), old);
            store.bind_turn_diff("turn".into(), old);
            let card = store.allocate_card();
            store.adopt_unkeyed(
                (0..count)
                    .map(|_| EntryDraft::plain(Role::User, "same"))
                    .collect(),
                mode,
            );
            assert_eq!(store.len(), count);
            assert!(store.entry(old).is_none());
            assert!(store.entry_id_for_item(&native).is_none());
            assert!(store.turn_diff("turn").is_none());
            assert_eq!(store.active_step(0), None);
            let ids: HashSet<_> = store.iter().map(|row| row.id).collect();
            assert_eq!(ids.len(), count);
            assert!(!ids.contains(&old));
            assert!(!ids.contains(&card));
            let next_card = store.allocate_card();
            assert_ne!(next_card, card);
            assert_ne!(next_card, old);
            assert!(!ids.contains(&next_card));
            store.assert_invariants();
        }
    }
}
