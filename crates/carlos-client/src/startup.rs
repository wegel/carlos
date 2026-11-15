//! Validate private saved-session pages against the exact read that requested them.

use std::collections::HashSet;

use carlos_proto::ids::SavedScopeId;
use carlos_proto::session::Backend;
use carlos_proto::startup::SavedPage;
use serde_json::Value;

use crate::ClientError;

#[derive(Clone)]
pub(crate) struct PageTarget {
    pub scope: Option<SavedScopeId>,
    pub backend: Option<Backend>,
}

pub(crate) fn reply(target: PageTarget, value: Value) -> Result<SavedPage, ClientError> {
    let page: SavedPage = serde_json::from_value(value)
        .map_err(|error| ClientError::InvalidPayload(error.to_string()))?;
    let mut choices = HashSet::new();
    if target
        .scope
        .as_ref()
        .is_some_and(|scope| scope != &page.scope)
        || target
            .backend
            .is_some_and(|backend| backend != page.backend)
        || page.can_archive != (page.backend == Backend::Codex)
        || page.rows.len() > 32
        || page.rows.iter().any(|row| {
            !choices.insert(&row.choice)
                || row.cwd.len() > 16384
                || row.name.as_ref().is_some_and(|name| name.len() > 512)
                || row.preview.len() > 4096
        })
    {
        return Err(ClientError::InvalidPayload(
            "saved-session page has another target or exceeds its metadata bounds".into(),
        ));
    }
    Ok(page)
}
