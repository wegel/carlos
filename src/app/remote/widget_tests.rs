//! Keep widget colors, footer labels and viewport anchors on the live device's draw path.

use carlos_core::session::models::Role;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use super::widget_fixture::device;
use super::Device;
use crate::app::dictation_state::DictationProfileState;
use crate::app::layout_state::TranscriptTheme;
use crate::app::tests::render_fixture;
use crate::app::tool_output_state::ToolOutputMode;

fn draw(device: &mut Device, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| device.draw(frame))
        .expect("live device draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

fn recording(device: &mut Device) {
    device.view.configure_dictation(DictationProfileState {
        id: "en".into(),
        name: "English".into(),
        model_label: Some("fixture-model".into()),
        model_usable: true,
        #[cfg(feature = "dictation")]
        model_path: Some(std::path::PathBuf::from("/tmp/widget-model.bin")),
        #[cfg(feature = "dictation")]
        language: Some("en".into()),
        #[cfg(feature = "dictation")]
        vocabulary: None,
    });
    device
        .view
        .dictation
        .start_recording()
        .expect("recording display facts");
}

#[test]
fn live_device_shows_dictation_indicator() {
    let mut device = device(Vec::new(), false);
    recording(&mut device);
    assert!(draw(&mut device, 120, 24).contains("DICTATING AUTO [English]"));
}

#[test]
fn live_device_shows_dictation_indicator_with_ralph_label() {
    let mut device = device(Vec::new(), true);
    recording(&mut device);
    let text = draw(&mut device, 120, 24);
    assert!(text.contains("DICTATING AUTO [English]"));
    assert!(text.contains("RALPH MODE"));
}

#[test]
fn live_device_shows_dictation_indicator_in_rewind_mode() {
    let mut device = device(vec![render_fixture::row(Role::User, "old prompt")], false);
    device.history.rewind_draft = Some("private draft".into());
    recording(&mut device);
    let text = draw(&mut device, 120, 24);
    assert!(device.history.rewind_draft.is_some());
    assert!(text.contains("DICTATING AUTO [English]"));
}

fn anchored_device() -> Device {
    let mut device = device(
        vec![
            render_fixture::row(Role::User, "Inspect these files"),
            render_fixture::row(Role::ToolCall, "→ Read a.rs"),
            render_fixture::row(Role::ToolCall, "→ Read b.rs"),
            render_fixture::row(Role::Assistant, "The files are ready."),
        ],
        false,
    );
    device.view.tool_output.mode = ToolOutputMode::Compact;
    device.view.viewport.auto_follow_bottom = false;
    device
}

fn layout(device: &mut Device, theme: TranscriptTheme, body_width: u16) {
    device.view.layout.themes.desktop = theme;
    device.view.layout.themes.phone = theme;
    let x = if theme.is_flow() { 1 } else { 2 };
    draw(device, body_width + x + 1, 4);
}

#[test]
fn switching_layouts_at_the_same_width_keeps_the_scrolled_message_visible() {
    let mut device = anchored_device();
    layout(&mut device, TranscriptTheme::Gutter, 50);
    device.view.viewport.scroll_top = device.view.render_cache.block_start(2).expect("read entry");
    device.remember_anchor();
    for theme in [
        TranscriptTheme::Flow,
        TranscriptTheme::Gutter,
        TranscriptTheme::Flow,
    ] {
        layout(&mut device, theme, 50);
        assert_eq!(
            device.view.viewport.scroll_top,
            device.view.render_cache.block_start(2).expect("read entry")
        );
        let body = device.view.viewport.scroll_top + device.view.render_cache.first_content_row(2);
        assert!(device
            .view
            .render_cache
            .rendered_line_at(body)
            .expect("anchored read")
            .text
            .contains("b.rs"));
    }
}

fn all_rows(device: &mut Device) -> Vec<crate::app::view_models::RenderedLine> {
    let transcript =
        super::transcript::Transcript::new(&device.client, &device.session, &device.documents)
            .expect("accepted transcript");
    let cache = &mut device.view.render_cache;
    let count = cache.rendered_line_count();
    cache.materialize_inputs(&transcript, 0, count.saturating_sub(1));
    (0..count)
        .map(|row| cache.rendered_line_at(row).expect("painted row").clone())
        .collect()
}

#[test]
fn resizing_restores_the_entry_anchor_when_reasoning_changes_spacing() {
    let mut device = device(
        vec![
            render_fixture::row(Role::ToolCall, "run first"),
            render_fixture::row(Role::Reasoning, "**Checking the palette**"),
            render_fixture::row(Role::ToolCall, "run second"),
        ],
        false,
    );
    device.view.layout.themes.desktop = TranscriptTheme::Gutter;
    device.view.layout.themes.phone = TranscriptTheme::Gutter;
    device.view.viewport.auto_follow_bottom = false;
    draw(&mut device, 53, 3);
    device.view.viewport.scroll_top = device.view.render_cache.block_start(2).expect("last tool");
    device.remember_anchor();
    for (width, blank) in [(18, true), (50, false), (18, true)] {
        draw(&mut device, width + 3, 3);
        let rows = all_rows(&mut device);
        assert_eq!(rows.len(), device.view.render_cache.rendered_line_count());
        for index in [1, 2] {
            let start = device.view.render_cache.block_start(index).expect("entry");
            assert_eq!(rows[start].separator, blank);
        }
        assert_eq!(
            device.view.viewport.scroll_top,
            device.view.render_cache.block_start(2).expect("last tool")
        );
    }
}

#[test]
fn mouse_selection_uses_the_same_gutter_as_the_terminal_after_resize() {
    let mut device = device(vec![render_fixture::row(Role::User, "abcdefghij")], false);
    for (width, height, content_x) in [(100, 32, 2), (54, 32, 1), (100, 18, 1), (100, 32, 2)] {
        draw(&mut device, width, height);
        let row = u16::from(device.view.layout.active.is_flow());
        let size = crate::app::view_models::TerminalSize {
            width: width as usize,
            height: height as usize,
        };
        for (kind, column) in [
            (MouseEventKind::Down(MouseButton::Left), content_x + 2),
            (MouseEventKind::Drag(MouseButton::Left), content_x + 4),
        ] {
            device
                .mouse_at(
                    MouseEvent {
                        kind,
                        column,
                        row,
                        modifiers: KeyModifiers::NONE,
                    },
                    size,
                )
                .expect("device mouse gesture");
        }
        let selection = device.view.viewport.selection.expect("drag selects text");
        assert_eq!(
            crate::app::selection::selected_text(selection, &all_rows(&mut device)),
            "cde"
        );
    }
}
