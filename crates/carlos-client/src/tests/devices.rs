//! Operator results remain outside conversations and reconnect never repeats a device write.

use carlos_proto::commands::Command;
use carlos_proto::devices::{DeviceAdd, DeviceInfo, DeviceRevoke};
use serde_json::json;

use super::client::{attached, command, greet, reply};
use crate::ReadReply;

#[test]
fn device_writes_recover_their_unscoped_result_without_reprovisioning_or_revoking_twice() {
    for revoke in [false, true] {
        let (mut client, connection) = attached();
        let id = "b".repeat(32).parse().expect("device");
        let outgoing = client
            .submit(connection, |operation| {
                if revoke {
                    Command::DeviceRevoke(DeviceRevoke {
                        operation,
                        device: id,
                    })
                } else {
                    Command::DeviceAdd(DeviceAdd {
                        operation,
                        device: DeviceInfo {
                            id,
                            name: "Phone".into(),
                        },
                        verifier: "a".repeat(64),
                    })
                }
            })
            .expect("operator write");
        let operation = match command(&outgoing).command {
            Command::DeviceAdd(request) => request.operation,
            Command::DeviceRevoke(request) => request.operation,
            _ => panic!("operator request"),
        };
        let _ = reply(
            &mut client,
            &outgoing,
            json!({"operation":operation,"state":{"kind":"pending"}}),
        );
        assert!(client
            .operations()
            .get(&operation)
            .expect("retained")
            .session
            .is_none());
        client.disconnect(connection);
        let (_, effects) = greet(&mut client, true, "d");
        assert!(effects
            .outgoing
            .iter()
            .any(|outgoing| matches!(command(outgoing).command, Command::Operation { .. })));
        assert!(effects.outgoing.iter().all(|outgoing| !matches!(
            command(outgoing).command,
            Command::DeviceAdd(_) | Command::DeviceRevoke(_)
        )));
    }
}

#[test]
fn device_list_replies_keep_their_correlation_and_reject_duplicate_metadata() {
    let (mut client, connection) = attached();
    let request = client
        .read(connection, Command::DeviceList {})
        .expect("list");
    let device = json!({"id":"b".repeat(32),"name":"Phone 🌍"});
    let effects = reply(&mut client, &request, json!({"devices":[device.clone()]}));
    assert_eq!(effects.replies[0].id, Some(request.id()));
    assert!(
        matches!(&effects.replies[0].reply, ReadReply::Devices(list) if list.devices[0].name == "Phone 🌍")
    );
    let request = client
        .read(connection, Command::DeviceList {})
        .expect("list again");
    assert!(client
        .receive(
            connection,
            crate::codec::Incoming::Success {
                id: request.id(),
                result: json!({"devices":[device.clone(),device]}),
            }
        )
        .is_err());
}
