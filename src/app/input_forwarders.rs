//! Connect background event producers before entering the terminal loop.

use std::sync::mpsc::Receiver;

use crate::app::AppState;
#[cfg(not(feature = "dictation"))]
use crate::event::spawn_event_forwarders;
#[cfg(feature = "dictation")]
use crate::event::spawn_event_forwarders_with_dictation;
use crate::event::UiEvent;

#[cfg(feature = "dictation")]
pub(super) fn wire_event_forwarders(
    app: &mut AppState,
    server_events_rx: std::sync::mpsc::Receiver<String>,
) -> Receiver<UiEvent> {
    let (dictation_tx, dictation_rx) = std::sync::mpsc::channel();
    let (summary_tx, summary_rx) = std::sync::mpsc::channel();
    app.configure_summary_worker(summary_tx);
    app.configure_dictation_event_sender(dictation_tx);
    spawn_event_forwarders_with_dictation(server_events_rx, dictation_rx, summary_rx)
}

#[cfg(not(feature = "dictation"))]
pub(super) fn wire_event_forwarders(
    app: &mut AppState,
    server_events_rx: std::sync::mpsc::Receiver<String>,
) -> Receiver<UiEvent> {
    let (summary_tx, summary_rx) = std::sync::mpsc::channel();
    app.configure_summary_worker(summary_tx);
    spawn_event_forwarders(server_events_rx, summary_rx)
}
