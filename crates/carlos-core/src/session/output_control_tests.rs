//! Split C1 and intermediate terminal escapes without leaking their payload into output.

use super::models::Role;
use super::state::SessionState;

#[test]
fn every_control_character_can_arrive_in_a_separate_live_fragment() {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    let key = state.backend_entry_key("tool");
    state.ensure_item_placeholder(&key, Role::ToolOutput);
    let entry = state.messages()[0].id.to_wire();
    let mut cursor = None;
    let mut text = String::new();
    let source = "hé\u{9d}title\u{9c}llo \u{90}payload\u{9c}🌍\u{98}hidden\u{9c}\n\x1b(Bplain\x1b[31m red\x1b[0m\u{9e}private\u{9c}\u{9f}private\u{9c}";
    for ch in source.chars() {
        state.messages.append_text(0, &ch.to_string(), None);
        let published = state.publish();
        let chunk = published
            .output_chunk(&entry, cursor.as_ref(), 4)
            .expect("read");
        assert!(!chunk.has_more);
        text.extend(chunk.segments.into_iter().map(|piece| piece.text));
        cursor = Some(chunk.next);
    }
    state.messages.complete(&key);
    let chunk = state
        .publish()
        .output_chunk(&entry, cursor.as_ref(), 4)
        .expect("finish");
    assert!(chunk.done);
    assert_eq!(text, "héllo 🌍\nplain red");
}
