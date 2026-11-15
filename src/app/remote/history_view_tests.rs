//! Keep an opaque rewind target stable while this device edits or scrolls its private draft.

use carlos_core::session::models::Role;
use carlos_proto::bytes::ByteLength;
use carlos_proto::prompts::{PromptRecord, PromptSelection, PromptSource};
use crossterm::event::Event;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::app::remote::widget_fixture::device;
use crate::app::remote::Device;
use crate::app::render_cache_state::TranscriptSource;
use crate::app::tests::render_fixture;

fn conversation() -> Device {
    device(
        vec![
            render_fixture::row(Role::User, "first"),
            render_fixture::row(Role::Assistant, "reply one"),
            render_fixture::row(Role::User, "second"),
            render_fixture::row(Role::Assistant, "reply two"),
            render_fixture::row(Role::Assistant, "tail"),
        ],
        false,
    )
}

fn select(device: &mut Device, index: usize, text: &str) -> PromptSelection {
    let entry = device
        .client
        .session(&device.session)
        .expect("accepted history")
        .entry_at(index)
        .expect("selected user entry")
        .id
        .clone();
    let selection = PromptSelection {
        session: device.session.clone(),
        source: PromptSource::Human,
        record: Some(PromptRecord {
            id: entry.clone(),
            entry: Some(entry),
            source: PromptSource::Human,
            bytes: ByteLength::new(text.len() as u64),
        }),
        has_older: index != 0,
        has_newer: index == 0,
    };
    device.history.selection = Some(selection.clone());
    device.history.rewind_draft = Some("private draft before rewind".into());
    device.history.align_pending = true;
    device.set_composer(text);
    selection
}

#[test]
fn rewind_edit_keeps_selected_anchor_for_fork() {
    let mut device = conversation();
    let selected = select(&mut device, 0, "first");
    device
        .event(None, Event::Paste(" edited".into()))
        .expect("edit private draft");
    assert_eq!(device.view.input.lines(), &["first edited"]);
    assert_eq!(device.history.selection.as_ref(), Some(&selected));
    assert!(device.submit_rewind(None, "first edited".into()).is_err());
    assert_eq!(device.history.selection.as_ref(), Some(&selected));
    assert_eq!(
        device.history.rewind_draft.as_deref(),
        Some("private draft before rewind")
    );
    assert!(device.client.operations().iter().next().is_none());
    let replica = device
        .client
        .session(&device.session)
        .expect("shared facts unchanged");
    assert_eq!(replica.entry_count(), 5);
    let rows = crate::app::remote::transcript::Transcript::new(
        &device.client,
        &device.session,
        &device.documents,
    )
    .expect("accepted rows");
    assert_eq!(rows.input(0).expect("first source").text, "first");
    assert_eq!(rows.input(1).expect("answer source").text, "reply one");
}

fn draw(device: &mut Device) -> String {
    let mut terminal = Terminal::new(TestBackend::new(100, 6)).expect("short terminal");
    terminal
        .draw(|frame| device.draw(frame))
        .expect("draw accepted history");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn rewind_scroll_aligns_to_selected_prompt_history_position() {
    let mut device = conversation();
    select(&mut device, 2, "second");
    let newer_screen = draw(&mut device);
    let newer_scroll = device.view.viewport.scroll_top;
    assert!(newer_screen.contains("second"));
    select(&mut device, 0, "first");
    let older_screen = draw(&mut device);
    let older_scroll = device.view.viewport.scroll_top;
    assert!(older_scroll <= newer_scroll);
    assert!(older_screen.contains("first"));
    assert!(!device.view.viewport.auto_follow_bottom);
    assert!(!device.history.align_pending);
    assert!(device.client.operations().iter().next().is_none());
}
