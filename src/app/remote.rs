//! Drive terminal widgets from one shared client connected to the local daemon.

mod approvals;
mod bodies;
mod dictation;
mod drafts;
mod events;
mod formatting;
mod goals;
mod goals_commands;
mod local_commands;
mod navigation;
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
mod queue_pages;
mod queue_reads;
mod queue_render;
mod reading;
mod render;
mod selection_copy;
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
mod status;
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
mod tasks_stop_results;
mod tasks_stops;
mod tasks_text;
mod tasks_windows;
mod transcript;
mod transport;

use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use carlos_client::Client;
use carlos_proto::ids::SessionId;
use crossterm::event::{self, Event, KeyCode};

use self::bodies::DocumentReads;
use self::transport::Transport;
use super::cli::CliOptions;
use super::terminal_ui::with_terminal;
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
    waiting_ready: bool,
}

/// Attach the terminal without creating or borrowing a native backend in this process.
pub(super) fn run(socket: &Path, options: &CliOptions) -> Result<()> {
    let session = options
        .resume_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("--daemon-socket currently requires resume SESSION_ID"))?
        .parse()?;
    let mut device = Device {
        client: Client::default(),
        session,
        view: ViewState::new(super::tool_output_preferences::load_summary_enabled()),
        documents: DocumentReads::default(),
        reading: reading::ReadingState::default(),
        goals: goals::Goals::default(),
        queue: queue::Queue::default(),
        tasks: tasks::Tasks::default(),
        questions: questions::Questions::default(),
        settings: settings::Settings::default(),
        status: status::StatusClock::default(),
        waiting_ready: false,
    };
    device.view.layout.themes = super::tool_output_preferences::load_themes();
    device.view.tool_output.mode = super::tool_output_preferences::load_mode();
    device.configure_dictation(options);
    device.run(socket)
}

impl Device {
    fn run(&mut self, socket: &Path) -> Result<()> {
        let mut transport = Some(Transport::connect(socket, &mut self.client)?);
        with_terminal(|terminal| {
            let mut redraw = true;
            loop {
                redraw |= self.expire_queue_mode_notice();
                redraw |= self.tick_task_countdowns();
                redraw |= self.tick_status_clock();
                if let Some(link) = transport.as_mut() {
                    match self.service(link) {
                        Ok(changed) => redraw |= changed,
                        Err(error) => {
                            self.connection_closed(link, &error);
                            transport = None;
                            redraw = true;
                        }
                    }
                }
                redraw |= self.question_url_tick();
                redraw |= self.source_tick(transport.as_mut());
                redraw |= self.poll_settings_appearance();
                if redraw {
                    terminal.draw(|frame| self.draw(frame))?;
                    if let Some(link) = transport.as_mut() {
                        self.request_visible(link)?;
                    }
                    redraw = false;
                }
                if !event::poll(Duration::from_millis(8))? {
                    redraw |= self.drain_dictation();
                    continue;
                }
                let input = event::read()?;
                let secret = self.client.session(&self.session).is_some_and(|replica| {
                    replica.approvals().iter().any(|card| card.input.is_some())
                });
                super::input_events::trace_terminal_event(&input, secret);
                if matches!(&input, Event::Key(key) if key.code == KeyCode::F(5))
                    && transport.is_none()
                {
                    match Transport::connect(socket, &mut self.client) {
                        Ok(link) => {
                            transport = Some(link);
                            self.view.status_hint = None;
                        }
                        Err(error) => self.view.status_hint = Some(error.to_string()),
                    }
                } else if self.event(transport.as_mut(), input)? {
                    return Ok(());
                }
                self.drain_dictation();
                self.remember_anchor();
                redraw = true;
            }
        })
    }

    fn connection_closed(&mut self, link: &Transport, error: &anyhow::Error) {
        self.client.disconnect(link.connection);
        self.reading.disconnected();
        self.documents.disconnected();
        self.goals.read = None;
        self.queue_disconnected();
        self.tasks_disconnected();
        self.settings_disconnected();
        self.view.status_hint = Some(format!("F5 reconnects: {error}"));
    }

    fn service(&mut self, link: &mut Transport) -> Result<bool> {
        let mut changed = false;
        for incoming in link.receive()? {
            changed |= self.receive(link, incoming)?;
            changed |= self.source_tick(Some(link));
        }
        changed |= self.drive_queue(link)?;
        changed |= self.drive_tasks(link)?;
        changed |= self.drive_settings(link)?;
        link.flush()?;
        Ok(changed)
    }
}
