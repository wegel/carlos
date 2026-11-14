//! Prove exact bounded source, format/entry binding, replacement and large plain-page recovery.

use carlos_proto::bytes::ByteOffset;
use carlos_proto::events::Event;
use carlos_proto::output::OutputRevision;
use carlos_proto::source::{SourceCursor, INLINE_SOURCE_BYTES};

use super::models::{MessageKind, Role};
use super::output::{OutputReadError, MAX_OUTPUT_BYTES};
use super::state::SessionState;

fn session() -> SessionState {
    SessionState::new("root".into(), Default::default(), false)
}

#[test]
fn source_seek_returns_a_bounded_utf8_window_without_reading_its_prefix() {
    let mut state = session();
    let source = format!("{}🌍tail", "x".repeat(8 * 1024 * 1024));
    state.append_message(Role::Assistant, &source);
    let id = state.messages()[0].id.to_wire();
    let batch = state.publish();
    for inside in 0..4 {
        let chunk = batch
            .source_at(
                &id,
                OutputRevision::new(0),
                ByteOffset::new(8 * 1024 * 1024 + inside),
                4,
            )
            .expect("direct seek");
        assert_eq!(chunk.offset.get(), 8 * 1024 * 1024);
        assert_eq!(chunk.text, "🌍");
        assert!(chunk.has_more);
        let tail = batch
            .source_chunk(&id, Some(&chunk.next), 4)
            .expect("continue after seek");
        assert_eq!(tail.text, "tail");
        assert!(tail.done);
    }
}

#[test]
fn source_seek_rejects_old_revisions_and_positions_past_the_body() {
    let mut state = session();
    state.append_message(Role::Assistant, "original");
    let id = state.messages()[0].id.to_wire();
    state.publish();
    assert_eq!(
        state
            .publish()
            .source_at(&id, OutputRevision::new(0), ByteOffset::new(u64::MAX), 4),
        Err(OutputReadError::InvalidCursor)
    );
    state.messages.update(
        0,
        Role::Assistant,
        Some("new body".into()),
        MessageKind::Plain,
        None,
    );
    assert_eq!(
        state
            .publish()
            .source_at(&id, OutputRevision::new(0), ByteOffset::new(0), 4),
        Err(OutputReadError::Replaced)
    );
    assert_eq!(
        state
            .publish()
            .source_at(&id, OutputRevision::new(1), ByteOffset::new(0), 4)
            .expect("current body")
            .text,
        "new "
    );
}

#[test]
fn exact_source_preserves_split_escapes_and_unicode_while_normalized_output_stays_safe() {
    let mut state = session();
    let key = state.backend_entry_key("colored");
    state.upsert_command_output_delta(&key, "");
    let id = state.messages()[0].id.to_wire();
    let mut source_cursor = None;
    let mut output_cursor = None;
    let mut original = String::new();
    let mut normalized = String::new();
    let source = "é\x1b[31m🌍red\x1b[0m\x1b]0;hidden\x07\n";
    for ch in source.chars() {
        state.upsert_command_output_delta(&key, &ch.to_string());
        let batch = state.publish();
        let chunk = batch
            .source_chunk(&id, source_cursor.as_ref(), 4)
            .expect("exact chunk");
        assert!(chunk.text.len() <= 4);
        original.push_str(&chunk.text);
        source_cursor = Some(chunk.next);
        let chunk = batch
            .output_chunk(&id, output_cursor.as_ref(), 4)
            .expect("safe output");
        normalized.extend(chunk.segments.into_iter().map(|piece| piece.text));
        output_cursor = Some(chunk.next);
    }
    assert_eq!(original, source);
    assert_eq!(normalized, "é🌍red\n");
}

#[test]
fn source_cursors_reject_foreign_rows_formats_invalid_offsets_and_replaced_plain_bodies() {
    let mut state = session();
    state.append_message(Role::Assistant, "hello 🌍");
    state.append_message(Role::Assistant, "other row");
    let id = state.messages()[0].id.to_wire();
    let other = state.messages()[1].id.to_wire();
    let next = state
        .publish()
        .source_chunk(&id, None, 4)
        .expect("first")
        .next;
    assert_eq!(
        state.publish().source_chunk(&other, Some(&next), 4),
        Err(OutputReadError::InvalidCursor)
    );
    for token in [
        next.as_str().replacen("s1:", "o1:", 1),
        format!("s1:{id}:{:016x}:{:016x}", 0, 7),
        format!("s1:{id}:{:016x}:{:016x}", 0, u64::MAX),
    ] {
        let cursor: SourceCursor = token.try_into().expect("bounded token");
        assert_eq!(
            state.publish().source_chunk(&id, Some(&cursor), 4),
            Err(OutputReadError::InvalidCursor)
        );
    }
    state.messages.update(
        0,
        Role::Assistant,
        Some("replacement".into()),
        MessageKind::Plain,
        None,
    );
    assert_eq!(
        state.publish().source_chunk(&id, Some(&next), 4),
        Err(OutputReadError::Replaced)
    );
}

#[test]
fn a_large_plain_row_uses_a_descriptor_and_bounded_chunks_without_losing_any_source() {
    let mut state = session();
    let source = "\x1b🌍\u{0}\n".repeat(2 * 1024 * 1024);
    state.append_message(Role::Assistant, &source);
    let batch = state.publish();
    let snapshot = batch.snapshot(100);
    let entry = &snapshot.entries[0];
    assert!(entry.text.is_empty());
    assert_eq!(
        entry.source.as_ref().expect("separate body").bytes.get(),
        source.len() as u64
    );
    assert!(serde_json::to_vec(&snapshot).expect("bounded facts").len() < 2048);
    let mut cursor = None;
    let mut actual = String::new();
    loop {
        let chunk = batch
            .source_chunk(&entry.id, cursor.as_ref(), usize::MAX)
            .expect("bounded source");
        assert!(chunk.text.len() <= MAX_OUTPUT_BYTES);
        actual.push_str(&chunk.text);
        cursor = Some(chunk.next);
        if !chunk.has_more {
            assert!(chunk.done);
            break;
        }
    }
    assert_eq!(actual, source);
}

#[test]
fn crossing_inline_limit_publishes_a_small_source_fact_instead_of_bulk_text() {
    let mut state = session();
    let key = state.backend_entry_key("answer");
    state.ensure_item_placeholder(&key, Role::Assistant);
    state.publish();
    state
        .messages
        .append_text(0, &"x".repeat(INLINE_SOURCE_BYTES + 1), None);
    let batch = state.publish();
    let record = batch
        .records()
        .iter()
        .find_map(|record| batch.to_wire(record))
        .expect("source event");
    assert!(matches!(record.event, Event::SourceChanged { .. }));
    assert!(serde_json::to_vec(&record).expect("encode").len() < 512);
}

#[test]
fn escaped_small_rows_shrink_pages_and_older_reads_recover_every_row_once() {
    let mut state = session();
    for _ in 0..1000 {
        state.append_message(Role::Assistant, "\0".repeat(INLINE_SOURCE_BYTES));
    }
    let batch = state.publish();
    let snapshot = batch.snapshot(1000);
    assert!(snapshot.has_older);
    assert!(snapshot.entries.len() < 1000);
    assert!(serde_json::to_vec(&snapshot).expect("encode").len() < 8 * 1024 * 1024);
    let mut count = snapshot.entries.len();
    let mut before = snapshot.entries.first().expect("newest page").id.clone();
    loop {
        let page = batch.history(&before, 1000).expect("older page");
        assert!(serde_json::to_vec(&page).expect("encode").len() < 8 * 1024 * 1024);
        count += page.entries.len();
        if !page.has_older {
            break;
        }
        before = page.entries.first().expect("progress").id.clone();
    }
    assert_eq!(count, 1000);
}
