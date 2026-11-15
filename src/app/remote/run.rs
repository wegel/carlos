//! Reuse a live greeted client when startup hands the terminal its logical session address.

use std::io;
use std::path::Path;

use anyhow::Result;
use carlos_client::Client;
use carlos_proto::ids::SessionId;
use crossterm::event::{self, Event, KeyCode};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use super::bodies::DocumentReads;
use super::goals::Goals;
use super::questions::Questions;
use super::queue::Queue;
use super::reading::ReadingState;
use super::settings::Settings;
use super::status::StatusClock;
use super::tasks::Tasks;
use super::transport::Transport;
use super::Device;
use crate::app::cli::CliOptions;
use crate::app::view_state::ViewState;

impl Device {
    pub(super) fn new(client: Client, session: SessionId, options: &CliOptions) -> Self {
        let mut device = Self {
            client,
            session,
            view: ViewState::new(crate::app::tool_output_preferences::load_summary_enabled()),
            documents: DocumentReads::default(),
            reading: ReadingState::default(),
            goals: Goals::default(),
            queue: Queue::default(),
            tasks: Tasks::default(),
            questions: Questions::default(),
            settings: Settings::default(),
            status: StatusClock::default(),
            ralph_options: carlos_proto::ralph::RalphOptions {
                prompt_path: options.ralph_prompt_path.clone(),
                done_marker: options.ralph_done_marker.clone(),
                blocked_marker: options.ralph_blocked_marker.clone(),
                waiting_marker: options.ralph_waiting_marker.clone(),
            },
            history: Default::default(),
            rename: None,
            waiting_ready: false,
            reconnect: None,
        };
        device.view.layout.themes = crate::app::tool_output_preferences::load_themes();
        device.view.tool_output.mode = crate::app::tool_output_preferences::load_mode();
        device.configure_dictation(options);
        device.configure_metrics();
        device
    }

    pub(super) fn run_connected(
        &mut self,
        socket: &Path,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        transport: Transport,
    ) -> Result<()> {
        let mut transport = Some(transport);
        let mut redraw = true;
        loop {
            self.metrics_loop();
            redraw |= self.expire_queue_mode_notice();
            redraw |= self.tick_task_countdowns();
            redraw |= self.tick_status_clock();
            redraw |= self.drive_transport(&mut transport);
            redraw |= self.question_url_tick();
            redraw |= self.source_tick(transport.as_mut());
            redraw |= self.poll_settings_appearance();
            if redraw {
                self.metrics_draw(terminal)?;
                if let Some(link) = transport.as_mut() {
                    self.request_visible(link)?;
                }
                redraw = false;
            }
            if !self.metrics_poll()? {
                redraw |= self.drain_dictation();
                continue;
            }
            if self.metrics_input(socket, &mut transport, event::read()?)? {
                return Ok(());
            }
            self.drain_dictation();
            self.remember_anchor();
            redraw = true;
        }
    }

    fn drive_transport(&mut self, transport: &mut Option<Transport>) -> bool {
        let Some(link) = transport.as_mut() else {
            return false;
        };
        match self.service(link) {
            Ok(changed) => changed,
            Err(error) => {
                self.connection_closed(link, &error);
                *transport = None;
                true
            }
        }
    }

    pub(super) fn event_with_reconnect(
        &mut self,
        socket: &Path,
        transport: &mut Option<Transport>,
        input: Event,
    ) -> Result<bool> {
        let secret = self.question_input_is_secret();
        crate::app::input_events::trace_terminal_event(&input, secret);
        if matches!(&input, Event::Key(key) if key.code == KeyCode::F(5))
            && self.recover_saved_draft()
        {
            return Ok(false);
        }
        if matches!(&input, Event::Key(key) if key.code == KeyCode::F(5)) && transport.is_none() {
            match Transport::connect(socket, &mut self.client) {
                Ok(link) => {
                    *transport = Some(link);
                    self.view.status_hint = None;
                }
                Err(error) => self.view.status_hint = Some(error.to_string()),
            }
            Ok(false)
        } else if matches!(&input, Event::Key(key) if key.code == KeyCode::F(5)) {
            if let Some(link) = transport.as_mut() {
                self.retry_session(link)?;
            }
            Ok(false)
        } else {
            self.event(transport.as_mut(), input)
        }
    }
}
