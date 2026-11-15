//! Launch and pick through typed daemon commands, then transfer the live client into the device.

mod actions;
mod archive;
mod options;
mod picker;
mod render;

use std::io;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use carlos_client::{Client, Effects, ReadReply};
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::{SavedChoiceId, SavedScopeId, SessionId};
use carlos_proto::submissions::{ClientOperation, OperationState};
use crossterm::event::{self, Event, KeyCode};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use self::options::Options;
use self::picker::Picker;
use super::transport::Transport;
use super::Device;
use crate::app::cli::CliOptions;
use crate::app::notifications::is_key_press_like;
use crate::app::terminal_ui::with_terminal;

enum Phase {
    Greeting,
    Listing,
    Picking,
    Opening,
    Archiving,
    Empty,
    Failed,
}

enum Submitted {
    Open(ClientOperation),
    Archive {
        operation: ClientOperation,
        scope: SavedScopeId,
        choice: SavedChoiceId,
    },
}

struct Bootstrap {
    client: Client,
    options: Options,
    phase: Phase,
    read: Option<RequestId>,
    submitted: Option<Submitted>,
    picker: Option<Picker>,
    error: Option<String>,
}

pub(super) fn run_connected(
    socket: &Path,
    options: &CliOptions,
    client: Client,
    transport: Transport,
) -> Result<()> {
    let startup = Bootstrap {
        client,
        options: Options::capture(options)?,
        phase: Phase::Greeting,
        read: None,
        submitted: None,
        picker: None,
        error: None,
    };
    let mut resumed = None;
    let mut report = None;
    let result = with_terminal(|terminal| {
        if let Some((client, transport, session)) = startup.run(socket, terminal, transport)? {
            let mut device = Device::new(client, session.clone(), options);
            resumed = Some(session);
            let result = device.run_connected(socket, terminal, transport);
            report = device.metrics_report();
            result?;
        }
        Ok(())
    });
    if let Some(session) = resumed {
        eprintln!("{}", crate::app::cli::daemon_resume_hint(socket, &session));
    }
    if let Some(report) = report {
        eprintln!("{report}");
    }
    result
}

impl Bootstrap {
    fn run(
        mut self,
        socket: &Path,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        transport: Transport,
    ) -> Result<Option<(Client, Transport, SessionId)>> {
        let mut transport = Some(transport);
        let mut redraw = true;
        loop {
            redraw |= self.read_transport(&mut transport);
            let ready = transport
                .as_ref()
                .is_some_and(|link| self.client.is_ready(link.connection));
            if let Some(session) = ready.then(|| self.accepted_address()).flatten() {
                let mut link = transport
                    .take()
                    .expect("open result has a live daemon transport");
                link.send(
                    self.client
                        .read(link.connection, Command::SavedClose { scope: None })?,
                )?;
                link.send(self.client.read(
                    link.connection,
                    Command::Subscribe {
                        session: session.clone(),
                        since: None,
                    },
                )?)?;
                return Ok(Some((self.client, link, session)));
            }
            if matches!(self.phase, Phase::Empty) {
                return Ok(None);
            }
            if redraw {
                terminal.draw(|frame| self.draw(frame))?;
                redraw = false;
            }
            if !event::poll(Duration::from_millis(8))? {
                continue;
            }
            if self.input(socket, &mut transport, terminal.size()?, event::read()?)? {
                return Ok(None);
            }
            redraw = true;
        }
    }

    fn read_transport(&mut self, transport: &mut Option<Transport>) -> bool {
        let Some(link) = transport.as_mut() else {
            return false;
        };
        match self.service(link) {
            Ok(changed) => changed,
            Err(error) => {
                self.client.disconnect(link.connection);
                self.error = Some(format!("F5 reconnects to the daemon: {error}"));
                if self.submitted.is_none() {
                    self.phase = Phase::Greeting;
                }
                if let Some(picker) = &mut self.picker {
                    picker.stale = true;
                }
                *transport = None;
                true
            }
        }
    }

    fn input(
        &mut self,
        socket: &Path,
        transport: &mut Option<Transport>,
        size: ratatui::layout::Size,
        input: Event,
    ) -> Result<bool> {
        crate::app::input_events::trace_terminal_event(&input, false);
        match input {
            Event::Key(key) if is_key_press_like(key.kind) => {
                if key.code == KeyCode::F(5) && transport.is_none() {
                    match Transport::connect(socket, &mut self.client) {
                        Ok(link) => {
                            *transport = Some(link);
                            self.error = None;
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    }
                } else {
                    return self.key(transport.as_mut(), key);
                }
            }
            Event::Mouse(mouse) => self.mouse(transport.as_mut(), size, mouse)?,
            _ => {}
        }
        Ok(false)
    }

    fn service(&mut self, link: &mut Transport) -> Result<bool> {
        self.ensure_started(link)?;
        let incoming = link.receive()?;
        let changed = !incoming.is_empty();
        for incoming in incoming {
            let effects = self.client.receive(link.connection, incoming)?;
            self.apply(link, effects)?;
        }
        self.ensure_started(link)?;
        self.settle_archive(link)?;
        link.flush()?;
        Ok(changed)
    }

    fn apply(&mut self, link: &mut Transport, effects: Effects) -> Result<()> {
        for outgoing in effects.outgoing {
            link.send(outgoing)?;
        }
        for result in effects.replies {
            if result.id == self.read {
                if let ReadReply::SavedPage(page) = result.reply {
                    self.read = None;
                    self.saved_page(link, page)?;
                }
            }
        }
        for failure in effects.failures {
            self.error = Some(failure.error.message);
            if failure.id == self.read {
                self.read = None;
                self.phase = if self.picker.is_some() {
                    Phase::Picking
                } else {
                    Phase::Failed
                };
            }
        }
        Ok(())
    }

    fn ensure_started(&mut self, link: &mut Transport) -> Result<()> {
        if self.client.is_ready(link.connection)
            && self.submitted.is_none()
            && matches!(self.phase, Phase::Greeting)
        {
            self.start(link)?;
        }
        Ok(())
    }

    fn accepted_address(&mut self) -> Option<SessionId> {
        let Some(Submitted::Open(operation)) = &self.submitted else {
            return None;
        };
        let result = self.client.operations().get(operation)?;
        match &result.state {
            OperationState::Pending | OperationState::Queued | OperationState::Accepted { .. } => {
                result.session.clone()
            }
            OperationState::Rejected { message, .. }
            | OperationState::Uncertain { message, .. } => {
                self.error = Some(message.clone());
                self.phase = Phase::Failed;
                None
            }
        }
    }
}
