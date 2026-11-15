//! Run one explicit session action without opening a terminal or creating a native owner.

use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use carlos_client::Client;
use carlos_proto::commands::{Command, SessionOperation};
use carlos_proto::ids::SessionId;
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState};

use super::cli::CliOptions;
use super::daemon_launcher;
use super::remote::Transport;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Stop,
    Reopen,
}

#[derive(Clone, Debug)]
pub(super) struct SessionControl {
    pub action: Action,
    pub session: SessionId,
}

impl Action {
    pub fn parse(text: &str) -> Result<Self> {
        match text {
            "stop" => Ok(Self::Stop),
            "reopen" => Ok(Self::Reopen),
            _ => bail!("Use session stop ID or session reopen ID"),
        }
    }

    pub fn command(self, session: SessionId, operation: ClientOperation) -> Command {
        let request = SessionOperation { session, operation };
        match self {
            Self::Stop => Command::Stop(request),
            Self::Reopen => Command::Reopen(request),
        }
    }
}

pub(super) fn parse(args: &mut impl Iterator<Item = String>) -> Result<SessionControl> {
    let action = Action::parse(
        &args
            .next()
            .context("Missing session action: stop or reopen")?,
    )?;
    let session = args
        .next()
        .context("Missing logical session ID")?
        .parse()
        .context("Invalid logical session ID")?;
    Ok(SessionControl { action, session })
}

pub(super) fn run(options: &CliOptions) -> Result<()> {
    let control = options.session_control.as_ref().expect("session command");
    let (mut client, mut link) = match &options.daemon_socket {
        Some(socket) => {
            let mut client = Client::default();
            let link = Transport::connect(socket, &mut client)?;
            (client, link)
        }
        None => daemon_launcher::connect_local()?,
    };
    let result = execute(control, &mut client, &mut link)?;
    match result.state {
        OperationState::Accepted { notice, .. } => {
            let fallback = match control.action {
                Action::Stop => "Session stopped",
                Action::Reopen => "Session reopened",
            };
            println!(
                "{}: {}",
                control.session,
                notice.as_deref().unwrap_or(fallback)
            );
            Ok(())
        }
        OperationState::Rejected { message, .. } => {
            bail!("{}: rejected: {message}", control.session)
        }
        OperationState::Uncertain { message, .. } => {
            bail!("{}: uncertain: {message}", control.session)
        }
        _ => unreachable!("terminal operation result"),
    }
}

fn execute(
    control: &SessionControl,
    client: &mut Client,
    link: &mut Transport,
) -> Result<OperationResult> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut operation = None;
    loop {
        link.flush()?;
        for incoming in link.receive()? {
            let effects = client.receive(link.connection, incoming)?;
            if let Some(failure) = effects.failures.first() {
                bail!("{}", failure.error.message);
            }
            for outgoing in effects.outgoing {
                link.send(outgoing)?;
            }
        }
        if operation.is_none() && client.is_ready(link.connection) {
            let outgoing = client.submit(link.connection, |id| {
                operation = Some(id.clone());
                control.action.command(control.session.clone(), id)
            })?;
            link.send(outgoing)?;
            link.flush()?;
        }
        if let Some(record) = operation
            .as_ref()
            .and_then(|id| client.operations().get(id))
        {
            if !matches!(
                record.state,
                OperationState::Pending | OperationState::Queued
            ) {
                return Ok(OperationResult {
                    operation: record.operation.clone(),
                    session: record.session.clone(),
                    rewound: record.rewound,
                    state: record.state.clone(),
                });
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            bail!("Session action was not confirmed within 30 seconds; no retry was sent");
        }
        link.wait(remaining)?;
    }
}
