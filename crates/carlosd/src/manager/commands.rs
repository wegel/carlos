//! Authenticate once, bind operation counters, and route only supported daemon methods.

use std::os::unix::ffi::OsStrExt;
use std::time::Instant;

use carlos_proto::commands::{check_protocol, Command, Hello, Request, RequestId, Sessions};
use carlos_proto::submissions::{OperationResult, OperationState};

use super::Manager;
use crate::command_info::{describe, Handler};
use crate::retry::{Admission, Resume};
use crate::transport::wire::error;

impl Manager {
    pub(super) fn command(&mut self, peer: u64, request: Request) {
        let Request { id, command, .. } = request;
        let Some(connection) = self.peers.get(&peer) else {
            return;
        };
        let info = describe(&command);
        if info.handler == Handler::Hello {
            self.hello(peer, id, command);
            return;
        }
        let Some(client) = connection.client.clone() else {
            self.fail(peer, id, error(-32002, "hello is required before commands"));
            return;
        };
        if let Err(reason) = self.retry.touch(&client, Instant::now()) {
            self.fail(peer, id, reason);
            return;
        }
        match info.handler {
            Handler::Hello => unreachable!("greeting handled before client lookup"),
            Handler::Devices => self.device_command(peer, id, command, client),
            Handler::Prompts => self.prompt_read_command(peer, id, command),
            Handler::Startup => self.startup_command(peer, id, command, client),
            Handler::Create => self.create(peer, id, command, client),
            Handler::Stop => self.stop_command(peer, id, command, client),
            Handler::Reopen => self.reopen_command(peer, id, command, client),
            Handler::Mutation => self.mutate(peer, id, command, client),
            Handler::Watch => self.watch_sessions(peer, id),
            Handler::Operation => {
                let Command::Operation { operation } = command else {
                    unreachable!("operation description")
                };
                match self.retry.read(&client, &operation, Instant::now()) {
                    Ok(result) => self.reply(peer, id, result),
                    Err(reason) => self.fail(peer, id, reason),
                }
            }
            Handler::Read => self.read_command(peer, id, command),
        }
    }

    fn watch_sessions(&mut self, peer: u64, id: RequestId) {
        self.peers.get_mut(&peer).expect("peer").list = true;
        let sessions = self
            .sessions
            .values()
            .map(|state| state.info.clone())
            .collect();
        self.reply(peer, id, Sessions { sessions });
    }

    fn hello(&mut self, peer: u64, id: RequestId, command: Command) {
        let Command::Hello(Hello {
            protocol,
            client,
            token,
            resume,
            resume_token,
            instance,
        }) = command
        else {
            unreachable!()
        };
        if self.peers[&peer].client.is_some() {
            self.fail(peer, id, error(-32002, "connection already greeted"));
            return;
        }
        if let Err(reason) = check_protocol(protocol) {
            self.fail(peer, id, reason);
            return;
        }
        if client.len() > 256 {
            self.fail(
                peer,
                id,
                error(
                    -32602,
                    "use a bounded client name and Unix peer credentials",
                ),
            );
            return;
        }
        let scope = match self.greeting_scope(peer, &token) {
            Ok(scope) => scope,
            Err(reason) => {
                self.fail(peer, id, reason);
                return;
            }
        };
        let resume = match resume_offer(resume, resume_token, instance) {
            Ok(resume) => resume,
            Err(reason) => {
                self.fail(peer, id, reason);
                return;
            }
        };
        match self.retry.hello_in(scope.clone(), resume, Instant::now()) {
            Ok(hello) => self.accept_hello(peer, id, hello, scope),
            Err(reason) => self.fail(peer, id, reason),
        }
    }

    fn accept_hello(
        &mut self,
        peer: u64,
        id: RequestId,
        mut hello: carlos_proto::commands::HelloResult,
        scope: crate::retry::ClientScope,
    ) {
        hello.identity = Some(self.daemon_identity());
        let connection = self.peers.get_mut(&peer).expect("peer");
        connection.client = Some(hello.client.clone());
        connection.scope = Some(scope);
        if matches!(
            connection.scope.as_ref(),
            Some(crate::retry::ClientScope::Device(_))
        ) {
            connection.socket.authorize_network();
        }
        connection.replay_allowed = hello.resumed;
        self.reply(peer, id, hello);
    }

    fn daemon_identity(&self) -> carlos_proto::identity::DaemonIdentity {
        carlos_proto::identity::DaemonIdentity {
            build: crate::build_info(),
            process: std::process::id(),
            codex_socket: self
                .codex_socket
                .as_ref()
                .map(|path| path.as_os_str().as_bytes().to_vec()),
        }
    }

    fn greeting_scope(
        &self,
        peer: u64,
        token: &carlos_proto::devices::AccessToken,
    ) -> Result<crate::retry::ClientScope, carlos_proto::commands::RpcError> {
        if self.peers[&peer].socket.is_network() {
            if self.peers[&peer].socket.network_login_expired() {
                return Err(error(-32002, "device greeting deadline expired"));
            }
            return self
                .authenticate_device(token)
                .map(crate::retry::ClientScope::Device);
        }
        if !token.is_empty() {
            return Err(error(
                -32602,
                "Unix greetings use peer credentials, not a device token",
            ));
        }
        Ok(crate::retry::ClientScope::Local)
    }

    fn mutate(
        &mut self,
        peer: u64,
        id: RequestId,
        command: Command,
        client: carlos_proto::ids::ClientId,
    ) {
        let canonical = serde_json::to_vec(&command).expect("typed command");
        let prepared = match self.prepare_mutation(peer, command) {
            Ok(parts) => parts,
            Err(reason) => {
                self.fail(peer, id, reason);
                return;
            }
        };
        let result = match self.retry.admit(
            &client,
            prepared.operation,
            Some(prepared.session.clone()),
            canonical,
            prepared.class,
            Instant::now(),
        ) {
            Ok(Admission::Existing(result)) => {
                self.reply(peer, id, result);
                return;
            }
            Ok(Admission::New(result)) => result,
            Err(reason) => {
                self.fail(peer, id, reason);
                return;
            }
        };
        let admitted = self
            .sessions
            .get(&prepared.session)
            .filter(|session| session.stopping.is_none())
            .and_then(|session| session.live.as_ref())
            .ok_or_else(|| error(-32012, "session has no live owner; subscribe to restore it"))
            .and_then(|live| live.handle.request(prepared.work));
        match admitted {
            Ok(_) => self.reply(peer, id, result),
            Err(reason) => {
                let result = OperationResult {
                    state: OperationState::Rejected {
                        message: reason.message,
                        recovery: prepared.recovery,
                    },
                    ..result
                };
                self.result(result.clone());
                self.reply(peer, id, result);
            }
        }
    }

    fn prepare_mutation(
        &self,
        peer: u64,
        command: Command,
    ) -> Result<super::mutation_work::Prepared, carlos_proto::commands::RpcError> {
        let window = self.task_command_context(peer, &command)?;
        let mut prepared = super::mutation_work::prepare(command)?;
        if let carlos_core::engine::EngineCommand::TaskStop { window: target, .. } =
            &mut prepared.work
        {
            *target = window;
        }
        Ok(prepared)
    }
}

fn resume_offer(
    client: Option<carlos_proto::ids::ClientId>,
    secret: Option<carlos_proto::ids::ResumeToken>,
    instance: Option<carlos_proto::ids::DaemonId>,
) -> Result<Option<Resume>, carlos_proto::commands::RpcError> {
    match (client, secret, instance) {
        (None, None, None) => Ok(None),
        (Some(client), Some(secret), Some(instance)) => Ok(Some(Resume {
            client,
            secret,
            instance,
        })),
        _ => Err(error(
            -32602,
            "resume requires client, private token and daemon instance",
        )),
    }
}
