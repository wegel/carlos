//! Drive terminal widgets from one shared client connected to the local daemon.

mod approvals;
mod bodies;
mod dictation;
mod drafts;
mod events;
mod formatting;
mod goals;
mod goals_commands;
mod help;
mod history;
mod history_keys;
mod history_operations;
mod lifecycle;
mod local_commands;
mod metrics;
mod mobile;
mod navigation;
mod paste;
mod questions;
mod questions_answers;
mod questions_keys;
mod questions_operations;
mod questions_render;
mod questions_url;
mod queue;
mod queue_commands;
mod queue_editor;
mod queue_outcomes;
mod queue_reads;
mod queue_render;
mod ralph_keys;
mod reading;
mod reconnect;
mod rename;
mod render;
mod run;
mod selection_copy;
#[cfg(test)]
mod session_name_tests;
mod settings;
mod settings_appearance;
mod settings_changes;
mod settings_context;
mod settings_intent;
mod settings_keys;
mod settings_render;
mod settings_results;
mod settings_save;
mod settings_submit;
mod source_copy;
mod source_formatter;
mod startup;
mod status;
mod submit;
mod tasks;
mod tasks_activity;
mod tasks_activity_reads;
mod tasks_activity_render;
mod tasks_details;
mod tasks_incoming;
mod tasks_move;
mod tasks_overview;
mod tasks_pages;
mod tasks_reads;
mod tasks_render;
#[cfg(test)]
mod tasks_selection_tests;
mod tasks_stop_results;
mod tasks_stops;
mod tasks_text;
mod tasks_windows;
mod transcript;
mod transport;
#[cfg(test)]
mod widget_fixture;
#[cfg(test)]
mod widget_tests;

use std::path::Path;

use anyhow::Result;
use carlos_client::Client;
use carlos_proto::ids::SessionId;

use self::bodies::DocumentReads;
pub(in crate::app) use self::transport::Transport;
use super::cli::CliOptions;
use super::view_state::ViewState;

struct Device {
    client: Client,
    session: SessionId,
    view: ViewState,
    documents: DocumentReads,
    reading: reading::ReadingState,
    goals: goals::Goals,
    queue: queue::Queue,
    tasks: tasks::Tasks,
    questions: questions::Questions,
    settings: settings::Settings,
    status: status::StatusClock,
    ralph_options: carlos_proto::ralph::RalphOptions,
    history: history::History,
    rename: Option<rename::Rename>,
    lifecycle: Option<lifecycle::Intent>,
    waiting_ready: bool,
    disconnected: Option<String>,
    reconnect: Option<carlos_proto::submissions::ClientOperation>,
}

/// Start or attach the terminal through the daemon that owns its native backend.
pub(super) fn run(socket: &Path, options: &CliOptions) -> Result<()> {
    let mut client = Client::default();
    let transport = Transport::connect(socket, &mut client)?;
    run_connected(socket, options, client, transport)
}

/// Reuse one live client and transport through startup and the session screen.
pub(super) fn run_connected(
    socket: &Path,
    options: &CliOptions,
    client: Client,
    transport: Transport,
) -> Result<()> {
    startup::run_connected(socket, options, client, transport)
}

impl Device {
    fn disconnect_hint(&self) -> Option<String> {
        self.disconnected
            .as_ref()
            .map(|error| format!("F5 reconnects: {error}"))
    }

    fn connection_closed(&mut self, link: &Transport, error: &anyhow::Error) {
        self.client.disconnect(link.connection);
        self.reading.disconnected();
        self.history.cancel_read();
        self.documents.disconnected();
        self.goals.read = None;
        self.queue_disconnected();
        self.tasks_disconnected();
        self.settings_disconnected();
        self.disconnected = Some(error.to_string());
        self.view.status_hint = None;
    }

    fn service(&mut self, link: &mut Transport) -> Result<bool> {
        let mut changed = false;
        for incoming in link.receive()? {
            changed |= self.metrics_receive(link, incoming)?;
            changed |= self.source_tick(Some(link));
        }
        changed |= self.drive_queue(link)?;
        changed |= self.drive_tasks(link)?;
        changed |= self.drive_settings(link)?;
        changed |= self.drive_reconnect();
        link.flush()?;
        Ok(changed)
    }
}

#[cfg(test)]
#[path = "remote/session_view_contract_tests.rs"]
mod session_view_contract_tests;
