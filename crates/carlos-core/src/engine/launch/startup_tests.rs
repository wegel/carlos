//! Reuse saved choices and the existing compatibility rules before non-Codex native startup.

use super::seed_settings;
use crate::session::runtime_settings::RuntimeDefaults;
use crate::session::runtime_startup::{claude_runtime_defaults, grok_runtime_defaults};
use crate::session::state::SessionState;

#[test]
fn claude_keeps_saved_explicit_choices_but_never_receives_a_reasoning_summary() {
    let mut session = SessionState::new("root".into(), saved("sonnet", "high"), false);
    seed_settings(&mut session, &claude_runtime_defaults());
    session.enable_claude();
    session.start_claude_settings();
    assert_eq!(
        session.next_turn_runtime_settings(),
        (Some("sonnet".into()), Some("high".into()), None)
    );
    assert!(!session.claude().inherits_model());
}

#[test]
fn claude_absent_choices_remain_native_and_grok_saved_choices_are_catalog_filtered() {
    let mut session = SessionState::new("root".into(), RuntimeDefaults::default(), false);
    seed_settings(&mut session, &claude_runtime_defaults());
    assert_eq!(session.next_turn_runtime_settings(), (None, None, None));
    let mut session = SessionState::new("root".into(), saved("grok-4.5", "unsupported"), false);
    session.set_available_models(crate::grok_backend::grok_model_catalog());
    seed_settings(&mut session, &grok_runtime_defaults());
    assert_eq!(
        session.next_turn_runtime_settings(),
        (Some("grok-4.5".into()), Some("high".into()), None)
    );
}

fn saved(model: &str, effort: &str) -> RuntimeDefaults {
    RuntimeDefaults {
        model: Some(model.into()),
        effort: Some(effort.into()),
        summary: Some("detailed".into()),
    }
}
