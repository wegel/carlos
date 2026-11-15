//! Read and save captured backend host defaults on an existing disk-capable worker.

use anyhow::{bail, Result};
use carlos_proto::context_defaults::{ContextChoice, ContextDefaults, ContextPolicy};
use carlos_proto::ids::SessionId;

use crate::backend::BackendKind;
use crate::context_preferences;

/// Read only on the worker; attached and unsupported backends need no file access.
pub(crate) fn read(
    session: SessionId,
    backend: BackendKind,
    attached: bool,
) -> Result<ContextDefaults> {
    let policy = policy(backend, attached, shell_override());
    read_with(session, policy, || context_preferences::load(backend))
}

/// Save the captured backend's host default without touching any native conversation.
pub(crate) fn save(backend: BackendKind, attached: bool, choice: ContextChoice) -> Result<()> {
    let policy = policy(backend, attached, shell_override());
    save_with(policy, choice, |tokens| {
        context_preferences::save(backend, tokens)
    })
}

fn policy(backend: BackendKind, attached: bool, shell_override: bool) -> ContextPolicy {
    match backend {
        BackendKind::Codex if attached => ContextPolicy::AttachedCodex,
        BackendKind::Claude if shell_override => ContextPolicy::ClaudeShellOverride,
        BackendKind::Grok => ContextPolicy::Unsupported,
        _ => ContextPolicy::Owned,
    }
}

fn read_with(
    session: SessionId,
    policy: ContextPolicy,
    load: impl FnOnce() -> Result<Option<u64>>,
) -> Result<ContextDefaults> {
    let saved_tokens = match policy {
        ContextPolicy::Owned | ContextPolicy::ClaudeShellOverride => load()?,
        ContextPolicy::AttachedCodex | ContextPolicy::Unsupported => None,
    };
    Ok(ContextDefaults {
        session,
        saved_tokens,
        policy,
    })
}

fn save_with(
    policy: ContextPolicy,
    choice: ContextChoice,
    write: impl FnOnce(Option<u64>) -> Result<()>,
) -> Result<()> {
    match policy {
        ContextPolicy::AttachedCodex => bail!("Attached Codex server owns the size"),
        ContextPolicy::Unsupported => bail!("this backend does not support saved context sizes"),
        _ => {}
    }
    let tokens = Option::<u64>::from(choice)
        .map(context_preferences::validate)
        .transpose()?;
    if policy == ContextPolicy::ClaudeShellOverride && tokens.is_some() {
        bail!("Unset CLAUDE_CODE_AUTO_COMPACT_WINDOW before choosing a size");
    }
    write(tokens)
}

fn shell_override() -> bool {
    std::env::var("CLAUDE_CODE_AUTO_COMPACT_WINDOW").is_ok_and(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "context_defaults_tests.rs"]
mod tests;
