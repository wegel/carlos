//! Numbered settings obey the same gap and replacement rules as transcript facts.

use carlos_proto::events::Event;
use carlos_proto::settings::{RuntimeChoices, SettingsFacts};

use super::{event, snapshot};
use crate::Replica;

fn facts(version: char) -> SettingsFacts {
    SettingsFacts {
        revision: version.to_string().repeat(64).parse().expect("revision"),
        current: RuntimeChoices {
            model: Some("model".into()),
            ..Default::default()
        },
        pending: RuntimeChoices::default(),
        supports_effort: true,
        supports_summary: false,
        model_count: 2,
        native_pending: false,
        legacy_overrides: false,
        label: "model".into(),
        summaries_enabled: false,
    }
}

#[test]
fn settings_follow_numbered_changes_and_duplicate_delivery_is_inert() {
    let mut replica = Replica::new(snapshot()).expect("replica");
    assert!(replica.settings().is_none());
    let next = event(
        11,
        Event::SettingsChanged {
            settings: Some(facts('a')),
        },
    );
    assert!(replica.apply(next.clone()).expect("next fact").settings);
    assert_eq!(replica.settings(), Some(&facts('a')));
    assert!(!replica.apply(next).expect("duplicate").settings);
    assert!(replica
        .apply(event(
            13,
            Event::SettingsChanged {
                settings: Some(facts('b'))
            }
        ))
        .is_err());
    assert_eq!(replica.settings(), Some(&facts('a')));
}

#[test]
fn snapshot_replaces_settings_atomically_and_accounts_retained_strings() {
    let mut replica = Replica::new(snapshot()).expect("replica");
    let empty_bytes = replica.retained_bytes();
    let mut replacement = snapshot();
    replacement.settings = Some(facts('c'));
    assert!(replica.replace(replacement).expect("replace").settings);
    assert_eq!(replica.settings(), Some(&facts('c')));
    assert!(replica.retained_bytes() >= empty_bytes + "model".len() * 2 + 1024);
}

#[test]
fn catalog_pages_preserve_private_positions_without_replacing_shared_settings() {
    let (mut client, connection) = super::client::attached();
    let command = carlos_proto::commands::Command::SettingsRead {
        session: super::session(),
        expected: Some(facts('a').revision),
        offset: 1,
        limit: Some(1),
    };
    let outgoing = client
        .read(connection, command)
        .expect("private catalog read");
    let page = catalog_page();
    let effects = super::client::reply(&mut client, &outgoing, serde_json::json!(page));
    assert!(matches!(&effects.replies[0].reply,
        crate::ReadReply::Settings(page) if page.offset == 1 && page.next.is_none()));
    assert!(client
        .session(&super::session())
        .expect("shared facts")
        .settings()
        .is_none());
}

#[test]
fn catalog_rejects_foreign_stale_shifted_and_nonprogressing_pages() {
    for field in ["session", "revision", "offset", "count", "next", "models"] {
        let target = crate::settings_reads::SettingsTarget {
            session: super::session(),
            expected: Some(facts('a').revision),
            offset: 1,
            limit: 1,
        };
        let mut page = catalog_page();
        match field {
            "session" => page.session = "f".repeat(32).parse().expect("foreign session"),
            "revision" => page.settings.revision = facts('b').revision,
            "offset" => page.offset = 0,
            "count" => page.settings.model_count = 0,
            "next" => page.next = Some(1),
            "models" => page.models.push(page.models[0].clone()),
            _ => unreachable!("fixed negative cases"),
        }
        assert!(
            crate::settings_reads::page(target, serde_json::json!(page)).is_err(),
            "{field}"
        );
    }
}

fn catalog_page() -> carlos_proto::settings::SettingsPage {
    carlos_proto::settings::SettingsPage {
        session: super::session(),
        seq: carlos_proto::ids::Sequence::new(10),
        settings: facts('a'),
        offset: 1,
        models: vec![carlos_proto::settings::ModelInfo {
            model: "model".into(),
            display_name: "Model 🌍".into(),
            supported_efforts: vec!["low".into(), "high".into()],
            default_effort: Some("low".into()),
            is_default: true,
        }],
        next: None,
    }
}

#[test]
fn tool_label_mutations_keep_private_outcomes_separate_from_shared_facts() {
    let (mut client, connection) = super::client::attached();
    let outgoing = client
        .submit(connection, |operation| {
            carlos_proto::commands::Command::ToolLabelsSet {
                session: super::session(),
                operation,
                expected: facts('a').revision,
                enabled: false,
                save_default: true,
            }
        })
        .expect("typed label mutation");
    let request: carlos_proto::commands::Request =
        serde_json::from_slice(outgoing.bytes()).expect("wire request");
    let carlos_proto::commands::Command::ToolLabelsSet {
        operation,
        enabled,
        save_default,
        ..
    } = request.command
    else {
        panic!("typed label request");
    };
    assert!(!enabled);
    assert!(save_default);
    let effects = super::client::reply(
        &mut client,
        &outgoing,
        serde_json::json!({
            "operation":operation,"session":super::session(),"state":{"kind":"accepted","entry":null,"notice":null}
        }),
    );
    assert_eq!(effects.operations, vec![operation]);
    assert!(client
        .session(&super::session())
        .expect("shared session")
        .settings()
        .is_none());
}
