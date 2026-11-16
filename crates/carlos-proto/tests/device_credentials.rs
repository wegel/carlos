//! Keep credentials out of diagnostics and reject unsafe or ambiguous operator metadata.

use carlos_proto::commands::{Command, Hello};
use carlos_proto::devices::{AccessToken, DeviceAdd, DeviceInfo, Devices, MAX_DEVICES};
use carlos_proto::ids::OperationId;
use carlos_proto::submissions::ClientOperation;
use serde_json::{json, Value};

fn device(index: usize) -> DeviceInfo {
    DeviceInfo {
        id: format!("{index:032x}").parse().expect("device id"),
        name: "Phone 🌍".into(),
    }
}

#[test]
fn first_hello_serializes_its_secret_but_debug_and_malformed_token_errors_do_not() {
    let secret = "b".repeat(64);
    let supplied = format!("{}.{}", device(1).id, secret);
    let command = Command::Hello(Hello {
        protocol: 1,
        client: "phone".into(),
        token: supplied.parse().expect("device credential"),
        resume: Some("c".repeat(32).parse().expect("client")),
        resume_token: Some("d".repeat(64).parse().expect("private retry secret")),
        instance: None,
    });
    let debug = format!("{command:?}");
    assert!(!debug.contains(&secret));
    assert!(!debug.contains(&"d".repeat(64)));
    let retry_token = command.clone();
    let Command::Hello(greeting) = retry_token else {
        panic!("hello")
    };
    assert!(
        !format!("{:?}", greeting.resume_token.expect("retry credential"))
            .contains(&"d".repeat(64))
    );
    let value = serde_json::to_value(&command).expect("wire greeting");
    assert_eq!(value["params"]["token"], supplied);
    assert_eq!(
        serde_json::from_value::<Command>(value).expect("decode"),
        command
    );
    for malformed in [
        secret.clone(),
        format!("{supplied}extra"),
        supplied.to_uppercase(),
    ] {
        let error = malformed
            .parse::<AccessToken>()
            .expect_err("malformed credential");
        assert!(!error.to_string().contains(&secret));
    }
    assert!(AccessToken::default().is_empty());
}

#[test]
fn operator_lists_preserve_names_but_reject_controls_duplicates_and_excess_rows() {
    let good = device(1);
    good.validate().expect("Unicode name");
    for name in [
        "".into(),
        " \t ".into(),
        "phone\nforged line".into(),
        "x".repeat(257),
    ] {
        assert!(DeviceInfo {
            name,
            ..good.clone()
        }
        .validate()
        .is_err());
    }
    assert!(Devices {
        devices: vec![good.clone(), good]
    }
    .validate()
    .is_err());
    let devices = (0..=MAX_DEVICES).map(device).collect();
    assert!(Devices { devices }.validate().is_err());
}

#[test]
fn provision_payload_cannot_hide_extra_fields_or_an_invalid_verifier() {
    let mut request = DeviceAdd {
        operation: ClientOperation {
            client: "c".repeat(32).parse().expect("client"),
            id: OperationId::new(1).expect("operation"),
        },
        device: device(1),
        verifier: "a".repeat(64),
    };
    request.validate().expect("bounded verifier");
    let mut value = serde_json::to_value(&request).expect("encode");
    value["secret"] = json!("not an accepted operator field");
    assert!(serde_json::from_value::<DeviceAdd>(value).is_err());
    request.verifier = "g".repeat(64);
    assert!(request.validate().is_err());
    let list = serde_json::to_value(Devices {
        devices: vec![device(1)],
    })
    .expect("safe list");
    assert_eq!(list["devices"][0].get("verifier"), None::<&Value>);
}
