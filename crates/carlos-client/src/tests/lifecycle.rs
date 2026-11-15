//! Keep lifecycle commands scoped and recover their results without replaying their writes.

use carlos_proto::commands::{Command, SessionOperation};
use carlos_proto::submissions::OperationState;
use serde_json::json;

use super::client::{attached, command, greet, reply};
use super::session;

#[test]
fn stop_and_reopen_remain_private_scoped_operations_without_prompt_echoes() {
    for reopen in [false, true] {
        let (mut client, connection) = attached();
        let outgoing = client
            .submit(connection, |operation| {
                let request = SessionOperation {
                    session: session(),
                    operation,
                };
                if reopen {
                    Command::Reopen(request)
                } else {
                    Command::Stop(request)
                }
            })
            .expect("explicit lifecycle");
        let (Command::Stop(request) | Command::Reopen(request)) = command(&outgoing).command else {
            panic!("typed");
        };
        let retained = client
            .operations()
            .get(&request.operation)
            .expect("private command");
        assert_eq!(retained.session, Some(session()));
        assert!(retained.text().is_none());
        reply(
            &mut client,
            &outgoing,
            json!({"operation":request.operation,"session":session(),"state":{"kind":"pending"}}),
        );
        client.disconnect(connection);
        let (_, effects) = greet(&mut client, true, "d");
        assert!(effects
            .outgoing
            .iter()
            .any(|outgoing| matches!(command(outgoing).command, Command::Operation { .. })));
        assert!(effects.outgoing.iter().all(|outgoing| !matches!(
            command(outgoing).command,
            Command::Stop(_) | Command::Reopen(_)
        )));
        assert!(matches!(
            client
                .operations()
                .get(&request.operation)
                .expect("pending remains")
                .state,
            OperationState::Pending | OperationState::Uncertain { .. }
        ));
    }
}
