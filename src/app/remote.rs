//! Drive terminal widgets from one shared client connected to the local daemon.

mod approvals;
mod bodies;
mod drafts;
mod events;
mod goals;
mod goals_commands;
mod local_commands;
mod navigation;
mod reading;
mod render;
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
        waiting_ready: false,
    };
    device.view.layout.themes = super::tool_output_preferences::load_themes();
    device.view.tool_output.mode = super::tool_output_preferences::load_mode();
    device.run(socket)
}

impl Device {
    fn run(&mut self, socket: &Path) -> Result<()> {
        let mut transport = Some(Transport::connect(socket, &mut self.client)?);
        with_terminal(|terminal| {
            let mut redraw = true;
            loop {
                if let Some(link) = transport.as_mut() {
                    match self.service(link) {
                        Ok(changed) => redraw |= changed,
                        Err(error) => {
                            self.client.disconnect(link.connection);
                            self.reading.disconnected();
                            self.goals.read = None;
                            self.view.status_hint = Some(format!("F5 reconnects: {error}"));
                            transport = None;
                            redraw = true;
                        }
                    }
                }
                if redraw {
                    terminal.draw(|frame| self.draw(frame))?;
                    if let Some(link) = transport.as_mut() {
                        self.request_visible(link)?;
                    }
                    redraw = false;
                }
                if !event::poll(Duration::from_millis(8))? {
                    continue;
                }
                let input = event::read()?;
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
                self.remember_anchor();
                redraw = true;
            }
        })
    }

    fn service(&mut self, link: &mut Transport) -> Result<bool> {
        let mut changed = false;
        for incoming in link.receive()? {
            changed |= self.receive(link, incoming)?;
        }
        link.flush()?;
        Ok(changed)
    }
}
