//! Exercise bounded output reads through live writes, control splits and replacement.

use carlos_proto::output::OutputCursor;
use carlos_proto::session::OutputKind;

use super::history_identity::{HistoryEntry, HistoryMode};
use super::models::{EntryDraft, MessageKind, Role, ToolOutputDetails};
use super::output::{OutputReadError, MAX_OUTPUT_BYTES, MAX_OUTPUT_SEGMENTS};
use super::state::SessionState;

fn session() -> SessionState {
    SessionState::new("root".into(), Default::default(), false)
}

fn drain(
    core: &mut SessionState,
    id: super::entry_id::EntryId,
    mut cursor: Option<OutputCursor>,
    limit: usize,
) -> (String, OutputCursor, usize) {
    let batch = core.publish();
    let mut text = String::new();
    let mut bytes = 0;
    loop {
        let chunk = batch
            .output_chunk(&id.to_wire(), cursor.as_ref(), limit)
            .expect("chunk");
        bytes += serde_json::to_vec(&chunk).expect("encode").len();
        assert!(chunk.segments.len() <= MAX_OUTPUT_SEGMENTS);
        for segment in &chunk.segments {
            text.push_str(&segment.text);
        }
        cursor = Some(chunk.next);
        if !chunk.has_more {
            break;
        }
    }
    (text, cursor.expect("cursor"), bytes)
}

#[test]
fn chunk_boundaries_preserve_unicode_newlines_and_split_terminal_sequences() {
    let mut core = session();
    let key = core.backend_entry_key("output");
    core.ensure_item_placeholder(&key, Role::ToolOutput);
    let id = core.messages[0].id;
    let mut cursor = None;
    let mut received = String::new();
    for fragment in [
        "hé",
        "llo 🌍\n",
        "\u{1b}",
        "[31",
        "mred\u{1b}",
        "[0m",
        "\u{1b}]title",
        "\u{1b}",
        "\\ok\n",
        "\u{1b}Pskip",
        "\u{1b}\\tail",
        "\u{9b}32mgreen\u{9b}0m",
        "\0\u{7}",
    ] {
        core.upsert_command_output_delta(&key, fragment);
        let (text, next, _) = drain(&mut core, id, cursor, 4);
        received.push_str(&text);
        cursor = Some(next);
    }
    core.mark_item_completed(&key);
    let (text, _, _) = drain(&mut core, id, cursor, 4);
    received.push_str(&text);
    assert_eq!(received, "héllo 🌍\nredok\ntailgreen");
    assert!(!received.contains('\u{1b}'));
}

#[test]
fn diff_prefixes_are_classified_correctly_even_one_source_fragment_at_a_time() {
    let mut core = session();
    let id = core.transcript_append(EntryDraft {
        role: Role::ToolOutput,
        text: "+++ file\n--- file\n@@ hunk\n+é\n-old\n context\n+".into(),
        kind: MessageKind::Diff,
        file_path: None,
        tool_facts: None,
        tool_output: None,
    });
    let batch = core.publish();
    let mut cursor = None;
    let mut pieces = Vec::new();
    loop {
        let chunk = batch
            .output_chunk(&id.to_wire(), cursor.as_ref(), 4)
            .expect("chunk");
        for segment in chunk.segments {
            for ch in segment.text.chars() {
                pieces.push((ch, segment.kind));
            }
        }
        cursor = Some(chunk.next);
        if !chunk.has_more {
            break;
        }
    }
    let text: String = pieces.iter().map(|(ch, _)| ch).collect();
    assert_eq!(text, "+++ file\n--- file\n@@ hunk\n+é\n-old\n context\n+");
    let kinds: Vec<_> = text
        .split_inclusive('\n')
        .scan(0, |offset, line| {
            let kind = pieces[*offset].1;
            *offset += line.chars().count();
            Some(kind)
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            OutputKind::Context,
            OutputKind::Context,
            OutputKind::Hunk,
            OutputKind::Addition,
            OutputKind::Deletion,
            OutputKind::Context,
            OutputKind::Addition
        ]
    );
}

#[test]
fn replacement_rejects_old_cursors_but_label_and_completion_keep_them() {
    let mut core = session();
    let key = core.backend_entry_key("output");
    core.ensure_item_placeholder(&key, Role::ToolOutput);
    core.upsert_command_output_delta(&key, "first");
    let id = core.messages[0].id;
    let (_, cursor, _) = drain(&mut core, id, None, 4);
    core.messages.set_tool(
        0,
        ToolOutputDetails {
            generated: true,
            summary: "Label".into(),
            failed: false,
        },
    );
    core.mark_item_completed(&key);
    assert!(
        core.publish()
            .output_chunk(&id.to_wire(), Some(&cursor), 4)
            .expect("same body")
            .done
    );
    core.messages.update(
        0,
        Role::ToolOutput,
        Some("replacement".into()),
        MessageKind::Plain,
        None,
    );
    let batch = core.publish();
    assert_eq!(
        batch
            .output_chunk(&id.to_wire(), Some(&cursor), 4)
            .expect_err("replaced"),
        OutputReadError::Replaced
    );
    assert_eq!(
        batch
            .output_chunk(&id.to_wire(), None, 64)
            .expect("restart")
            .segments[0]
            .text,
        "replacement"
    );
}

#[test]
fn same_thread_history_preserves_or_changes_revisions_by_actual_body() {
    let mut core = session();
    let key = core.backend_entry_key("output");
    core.upsert_command_output_delta(&key, "body");
    let id = core.messages[0].id;
    let (_, cursor, _) = drain(&mut core, id, None, 4);
    for (text, valid) in [("body", true), ("changed", false)] {
        core.replace_history(
            vec![HistoryEntry {
                draft: EntryDraft::plain(Role::ToolOutput, text),
                key: Some(key.clone()),
                occurrences: 1,
            }],
            HistoryMode::SameThread,
        );
        assert_eq!(core.messages[0].id, id);
        assert_eq!(
            core.publish()
                .output_chunk(&id.to_wire(), Some(&cursor), 4)
                .is_ok(),
            valid
        );
    }
}

#[test]
fn stale_foreign_malformed_and_mid_codepoint_cursors_fail_explicitly() {
    let mut core = session();
    let first = core.transcript_append(EntryDraft::plain(Role::ToolOutput, "é"));
    let second = core.transcript_append(EntryDraft::plain(Role::ToolOutput, "other"));
    let batch = core.publish();
    let cursor = batch
        .output_chunk(&first.to_wire(), None, 4)
        .expect("cursor")
        .next;
    assert_eq!(
        batch
            .output_chunk(&second.to_wire(), Some(&cursor), 4)
            .expect_err("foreign"),
        OutputReadError::InvalidCursor
    );
    let bad: OutputCursor = "malformed".to_owned().try_into().expect("opaque token");
    assert_eq!(
        batch
            .output_chunk(&first.to_wire(), Some(&bad), 4)
            .expect_err("malformed"),
        OutputReadError::InvalidCursor
    );
    let mut parts: Vec<_> = cursor.as_str().split(':').map(str::to_owned).collect();
    parts[3] = "0000000000000001".into();
    let middle: OutputCursor = parts.join(":").try_into().expect("opaque token");
    assert_eq!(
        batch
            .output_chunk(&first.to_wire(), Some(&middle), 4)
            .expect_err("utf8"),
        OutputReadError::InvalidCursor
    );
}

#[test]
fn long_live_lines_and_unterminated_controls_consume_linear_bytes() {
    let mut core = session();
    let key = core.backend_entry_key("output");
    core.ensure_item_placeholder(&key, Role::ToolOutput);
    let id = core.messages[0].id;
    let mut cursor = None;
    let mut received = 0;
    let mut transferred = 0;
    for fragment in std::iter::repeat_n("a".repeat(1024), 256) {
        core.upsert_command_output_delta(&key, &fragment);
        let (text, next, bytes) = drain(&mut core, id, cursor, 1024);
        received += text.len();
        transferred += bytes;
        cursor = Some(next);
    }
    assert_eq!(received, 256 * 1024);
    assert!(
        transferred < 2 * received,
        "{transferred} bytes for {received} source bytes"
    );
    core.upsert_command_output_delta(&key, "\u{1b}]");
    let (_, next, _) = drain(&mut core, id, cursor, 1024);
    cursor = Some(next);
    for _ in 0..256 {
        core.upsert_command_output_delta(&key, &"x".repeat(1024));
        let (text, next, bytes) = drain(&mut core, id, cursor, 1024);
        assert!(text.is_empty());
        assert!(bytes < 1024);
        cursor = Some(next);
    }
    core.mark_item_completed(&key);
    assert!(
        core.publish()
            .output_chunk(&id.to_wire(), cursor.as_ref(), MAX_OUTPUT_BYTES)
            .expect("final")
            .done
    );
}
