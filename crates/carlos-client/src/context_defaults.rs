//! Validate one original host-default read without replacing live session usage facts.

use carlos_proto::context_defaults::{
    ContextDefaults, ContextPolicy, MAX_CONTEXT_TOKENS, MIN_CONTEXT_TOKENS,
};
use carlos_proto::ids::SessionId;
use serde_json::Value;

use crate::client_replies::parse;
use crate::ClientError;

pub(crate) fn reply(session: SessionId, value: Value) -> Result<ContextDefaults, ClientError> {
    if value.get("saved_tokens").is_none() {
        return Err(ClientError::InvalidPayload(
            "context reply omitted its saved choice".into(),
        ));
    }
    let defaults: ContextDefaults = parse(value)?;
    if defaults.session != session {
        return Err(ClientError::ForeignSession);
    }
    let invalid_tokens = defaults.saved_tokens.is_some_and(|tokens| {
        !(MIN_CONTEXT_TOKENS..=MAX_CONTEXT_TOKENS).contains(&tokens)
            || matches!(
                defaults.policy,
                ContextPolicy::AttachedCodex | ContextPolicy::Unsupported
            )
    });
    if invalid_tokens {
        return Err(ClientError::InvalidPayload(
            "saved context default is invalid for its policy".into(),
        ));
    }
    Ok(defaults)
}
