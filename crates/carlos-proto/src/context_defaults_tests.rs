//! Keep Auto explicit and reject malformed token choices before daemon dispatch.

use serde_json::json;

use super::ContextChoice;

#[test]
fn choices_round_trip_without_an_implicit_reset_value() {
    for (choice, value) in [
        (ContextChoice::Auto, json!({"kind":"auto"})),
        (
            ContextChoice::Tokens(500_000),
            json!({"kind":"tokens", "tokens":500_000}),
        ),
    ] {
        assert_eq!(serde_json::to_value(choice).expect("choice JSON"), value);
        assert_eq!(
            serde_json::from_value::<ContextChoice>(value).expect("typed choice"),
            choice
        );
        assert_eq!(ContextChoice::from(Option::<u64>::from(choice)), choice);
    }
    for value in [
        json!(null),
        json!({}),
        json!({"kind":"tokens"}),
        json!({"kind":"tokens", "tokens":-1}),
        json!({"kind":"tokens", "tokens":1.5}),
        json!({"kind":"auto", "tokens":500_000}),
        json!({"kind":"unknown"}),
        json!({"kind":"auto", "extra":1}),
    ] {
        assert!(
            serde_json::from_value::<ContextChoice>(value.clone()).is_err(),
            "{value}"
        );
    }
}
