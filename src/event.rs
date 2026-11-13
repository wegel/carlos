//! Unified event channel merging terminal input and server-line streams.

use std::sync::mpsc;
use std::thread;

use carlos_core::backend::EventReceiver;
use crossterm::event::{self, Event as CrosstermEvent};

#[cfg(feature = "dictation")]
use crate::dictation::events::DictationEvent;

/// A single event delivered to the main UI loop.
#[derive(Debug)]
pub(crate) enum UiEvent {
    Terminal(CrosstermEvent),
    ServerLine(String),
    Summary(carlos_core::summaries::SummaryEvent),
    #[cfg(feature = "dictation")]
    Dictation(DictationEvent),
}

#[cfg(not(feature = "dictation"))]
pub(crate) fn spawn_event_forwarders(
    mut server_events_rx: EventReceiver,
    summary_rx: mpsc::Receiver<carlos_core::summaries::SummaryEvent>,
) -> EventReceiver<UiEvent> {
    let (ui_tx, ui_rx) = mpsc::channel::<UiEvent>();
    let close = server_events_rx.take_close();
    let ui_rx = EventReceiver::with_close(ui_rx, move || {
        if let Some(close) = close {
            close();
        }
    });
    forward_summaries(summary_rx, ui_tx.clone());

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
        thread::spawn(move || {
            while let Ok(ev) = event::read() {
                if tx.send(UiEvent::Terminal(ev)).is_err() {
                    break;
                }
            }
        });
    }

    drop(ui_tx);
    ui_rx
}

#[cfg(feature = "dictation")]
pub(crate) fn spawn_event_forwarders_with_dictation(
    mut server_events_rx: EventReceiver,
    dictation_events_rx: mpsc::Receiver<DictationEvent>,
    summary_rx: mpsc::Receiver<carlos_core::summaries::SummaryEvent>,
) -> EventReceiver<UiEvent> {
    let (ui_tx, ui_rx) = mpsc::channel::<UiEvent>();
    let close = server_events_rx.take_close();
    let ui_rx = EventReceiver::with_close(ui_rx, move || {
        if let Some(close) = close {
            close();
        }
    });
    forward_summaries(summary_rx, ui_tx.clone());

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
        thread::spawn(move || {
            while let Ok(event) = dictation_events_rx.recv() {
                if tx.send(UiEvent::Dictation(event)).is_err() {
                    break;
                }
            }
        });
    }

    {
        let tx = ui_tx.clone();
        thread::spawn(move || {
            while let Ok(ev) = event::read() {
                if tx.send(UiEvent::Terminal(ev)).is_err() {
                    break;
                }
            }
        });
    }

    drop(ui_tx);
    ui_rx
}

fn forward_summaries(
    rx: mpsc::Receiver<carlos_core::summaries::SummaryEvent>,
    tx: mpsc::Sender<UiEvent>,
) {
    thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            if tx.send(UiEvent::Summary(event)).is_err() {
                break;
            }
        }
    });
}
