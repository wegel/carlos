//! Unified event channel merging terminal input and server-line streams.

use std::sync::mpsc;
use std::thread;

use crossterm::event::{self, Event as CrosstermEvent};

/// A single event delivered to the main UI loop.
#[derive(Debug)]
pub(crate) enum UiEvent {
    Terminal(CrosstermEvent),
    ServerLine(String),
}

pub(crate) fn spawn_event_forwarders(
    server_events_rx: mpsc::Receiver<String>,
) -> mpsc::Receiver<UiEvent> {
    let (ui_tx, ui_rx) = mpsc::channel::<UiEvent>();

    {
        let tx = ui_tx.clone();
        thread::spawn(move || {
            while let Ok(line) = server_events_rx.recv() {
                if tx.send(UiEvent::ServerLine(line)).is_err() {
                    break;
                }
            }
        });
    }

    {
        let tx = ui_tx.clone();
        thread::spawn(move || loop {
            let Ok(ev) = event::read() else {
                break;
            };
            if tx.send(UiEvent::Terminal(ev)).is_err() {
                break;
            }
        });
    }

    drop(ui_tx);
    ui_rx
}
