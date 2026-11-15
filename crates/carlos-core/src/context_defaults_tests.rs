//! Keep host context policies explicit without changing process environment or user files.

use std::cell::Cell;

use anyhow::anyhow;
use carlos_proto::context_defaults::{ContextChoice, ContextPolicy};
use carlos_proto::ids::SessionId;

use super::{policy, read_with, resolve_shell_override, save_with};
use crate::backend::BackendKind;
use crate::session::runtime_settings::RuntimeDefaults;
use crate::session::state::SessionState;

#[test]
fn child_context_environment_preserves_absent_empty_and_nonempty_values() {
    let mut session = SessionState::new("native-thread".into(), RuntimeDefaults::default(), false);
    assert_eq!(
        session.context_shell_override(),
        None,
        "legacy caller inherits on the worker"
    );
    for (effective, expected) in [
        (None, false),
        (Some(""), false),
        (Some("200000"), true),
        (Some(" "), true),
        (Some("窗"), true),
    ] {
        session.configure_context_shell_override(effective);
        assert_eq!(session.context_shell_override(), Some(expected));
    }
}

#[test]
fn captured_child_context_policy_never_reads_a_different_daemon_environment() {
    for (captured, expected) in [
        (false, ContextPolicy::Owned),
        (true, ContextPolicy::ClaudeShellOverride),
    ] {
        let resolved = resolve_shell_override(Some(captured), || panic!("child value is known"));
        let actual = policy(BackendKind::Claude, false, resolved);
        assert_eq!(actual, expected);
        let written = Cell::new(false);
        let result = save_with(actual, ContextChoice::Tokens(500_000), |_| {
            written.set(true);
            Ok(())
        });
        assert_eq!(result.is_ok(), !captured);
        assert_eq!(written.get(), !captured);
    }
}

#[test]
fn legacy_context_policy_reads_inherited_environment_only_when_child_value_is_unknown() {
    for inherited in [false, true] {
        let calls = Cell::new(0);
        let resolved = resolve_shell_override(None, || {
            calls.set(calls.get() + 1);
            inherited
        });
        assert_eq!(resolved, inherited);
        assert_eq!(calls.get(), 1);
    }
}

#[test]
fn policy_distinguishes_owned_attached_shell_and_unsupported_backends() {
    assert_eq!(
        policy(BackendKind::Codex, false, false),
        ContextPolicy::Owned
    );
    assert_eq!(
        policy(BackendKind::Codex, true, false),
        ContextPolicy::AttachedCodex
    );
    assert_eq!(
        policy(BackendKind::Claude, false, true),
        ContextPolicy::ClaudeShellOverride
    );
    assert_eq!(
        policy(BackendKind::Claude, false, false),
        ContextPolicy::Owned
    );
    assert_eq!(
        policy(BackendKind::Grok, false, false),
        ContextPolicy::Unsupported
    );
}

#[test]
fn attached_and_unsupported_reads_never_touch_a_saved_file() {
    for policy in [ContextPolicy::AttachedCodex, ContextPolicy::Unsupported] {
        let result = read_with(session(), policy, || {
            panic!("read-only default needs no file read")
        })
        .expect("policy reply");
        assert_eq!(result.policy, policy);
        assert_eq!(result.saved_tokens, None);
    }
}

#[test]
fn owned_and_shell_reads_preserve_saved_values_and_explicit_file_errors() {
    for policy in [ContextPolicy::Owned, ContextPolicy::ClaudeShellOverride] {
        assert_eq!(
            read_with(session(), policy, || Ok(Some(500_000)))
                .expect("saved default")
                .saved_tokens,
            Some(500_000)
        );
        assert_eq!(
            read_with(session(), policy, || Ok(None))
                .expect("Auto default")
                .saved_tokens,
            None
        );
        assert!(read_with(session(), policy, || Err(anyhow!("invalid saved context"))).is_err());
    }
}

#[test]
fn forbidden_and_invalid_choices_fail_before_any_file_write() {
    for (policy, choice) in [
        (ContextPolicy::AttachedCodex, ContextChoice::Auto),
        (ContextPolicy::AttachedCodex, ContextChoice::Tokens(500_000)),
        (ContextPolicy::Unsupported, ContextChoice::Auto),
        (
            ContextPolicy::ClaudeShellOverride,
            ContextChoice::Tokens(500_000),
        ),
        (ContextPolicy::Owned, ContextChoice::Tokens(99_999)),
        (ContextPolicy::Owned, ContextChoice::Tokens(1_000_001)),
    ] {
        let result = save_with(policy, choice, |_| panic!("rejected choice cannot write"));
        assert!(result.is_err(), "{policy:?} {choice:?}");
    }
}

#[test]
fn owned_and_shell_auto_writes_keep_the_exact_captured_value_or_disk_error() {
    let written = Cell::new(None);
    save_with(
        ContextPolicy::Owned,
        ContextChoice::Tokens(123_456),
        |tokens| {
            written.set(tokens);
            Ok(())
        },
    )
    .expect("exact size");
    assert_eq!(written.get(), Some(123_456));
    save_with(
        ContextPolicy::ClaudeShellOverride,
        ContextChoice::Auto,
        |tokens| {
            written.set(tokens);
            Ok(())
        },
    )
    .expect("shell override permits Auto");
    assert_eq!(written.get(), None);
    let error = save_with(ContextPolicy::Owned, ContextChoice::Auto, |_| {
        Err(anyhow!("disk rejected write"))
    })
    .expect_err("disk failure reaches original caller");
    assert_eq!(error.to_string(), "disk rejected write");
}

fn session() -> SessionId {
    "11111111111111111111111111111111"
        .parse()
        .expect("logical session")
}
