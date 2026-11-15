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
