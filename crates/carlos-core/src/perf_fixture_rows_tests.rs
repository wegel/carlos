//! Keep full offline fixtures, exact originals and deterministic display facts across the export seam.

use std::io::Cursor;

use carlos_proto::session::{EntryFormat, EntryKind};
use serde_json::json;

use super::{replay_perf_rows, synthetic_perf_rows};
use crate::perf_fixture::{populate_synthetic_perf_session, SyntheticPerfSpec};
use crate::session::models::Role;
use crate::session::perf_rows::consume;
use crate::session::state::SessionState;

#[test]
fn synthetic_export_keeps_more_than_a_wire_page_and_every_source_byte() {
    let spec = SyntheticPerfSpec {
        seed: 7,
        turns: 200,
        tool_output_lines: 8,
    };
    let left = synthetic_perf_rows(spec, |_| {});
    let right = synthetic_perf_rows(spec, |_| {});
    assert_eq!(left.rows.len(), 1200);
    assert_eq!(left.relevant_items, 1200);
    for (a, b) in left.rows.iter().zip(&right.rows) {
        assert_ne!(a.entry.id, b.entry.id);
        assert_eq!(
            (a.entry.kind, a.entry.format, a.text()),
            (b.entry.kind, b.entry.format, b.text())
        );
        if let Some((source, text)) = &a.source {
            assert_eq!(source.bytes.get(), text.len() as u64);
            assert!(a.entry.text.is_empty());
        }
    }
}

#[test]
fn exported_rows_move_inline_and_large_original_allocations() {
    let mut session = SessionState::new("fixture".into(), Default::default(), false);
    session.append_message(Role::User, "日本語 original");
    session.append_message(Role::ToolOutput, "\x1b[31m🌍\x1b[0m".repeat(1000));
    let pointers: Vec<_> = session
        .messages()
        .iter()
        .map(|row| row.text.as_ptr())
        .collect();
    let rows = consume(session);
    assert_eq!(rows[0].text().as_ptr(), pointers[0]);
    assert_eq!(rows[1].text().as_ptr(), pointers[1]);
    assert_eq!(rows[0].entry.kind, EntryKind::User);
    assert!(rows[1].source.is_some());
}

#[test]
fn captured_replay_preserves_unicode_markdown_and_reasoning_facts() {
    let records = [
        json!({"type":"response_item","payload":{"type":"message","role":"user","text":"日本語 🌍"}}),
        json!({"type":"response_item","payload":{"type":"message","role":"assistant","text":"**bold**\n\n[link](https://example.test)"}}),
        json!({"type":"response_item","payload":{"type":"reasoning","summary":[{"text":"thinking é"}]}}),
    ];
    let data = records
        .iter()
        .map(|value| value.to_string() + "\n")
        .collect::<String>();
    let mut samples = 0;
    let fixture = replay_perf_rows(Cursor::new(data), |_| samples += 1).expect("offline records");
    assert_eq!(samples, 3);
    assert_eq!(fixture.relevant_items, 3);
    assert_eq!(fixture.rows[0].text(), "日本語 🌍");
    assert_eq!(
        fixture.rows[1].text(),
        "**bold**\n\n[link](https://example.test)"
    );
    assert_eq!(fixture.rows[2].text(), "thinking é");
    assert_eq!(fixture.rows[2].entry.format, EntryFormat::Plain);
}

#[test]
fn synthetic_perf_messages_are_reproducible() {
    let spec = SyntheticPerfSpec {
        seed: 7,
        turns: 12,
        tool_output_lines: 8,
    };
    let left = synthetic_session(spec);
    let right = synthetic_session(spec);

    assert_eq!(left.messages().len(), spec.turns * 6);
    assert_eq!(left.messages().len(), right.messages().len());
    for (index, (lhs, rhs)) in left
        .messages()
        .iter()
        .zip(right.messages().iter())
        .enumerate()
    {
        assert_ne!(lhs.id, rhs.id);
        assert_eq!(left.messages().position(lhs.id), Some(index));
        assert_eq!(lhs.role, rhs.role);
        assert_eq!(lhs.kind, rhs.kind);
        assert_eq!(lhs.text, rhs.text);
        assert_eq!(lhs.file_path, rhs.file_path);
    }
}

#[test]
fn synthetic_perf_messages_change_with_seed() {
    let base = synthetic_session(SyntheticPerfSpec {
        seed: 7,
        turns: 4,
        tool_output_lines: 4,
    });
    let changed = synthetic_session(SyntheticPerfSpec {
        seed: 8,
        turns: 4,
        tool_output_lines: 4,
    });

    assert_eq!(base.messages().len(), changed.messages().len());
    assert!(base
        .messages()
        .iter()
        .zip(changed.messages().iter())
        .any(|(lhs, rhs)| lhs.text != rhs.text));
}

fn synthetic_session(spec: SyntheticPerfSpec) -> SessionState {
    let mut session = SessionState::new("perf".into(), Default::default(), false);
    populate_synthetic_perf_session(spec, &mut session);
    session
}
