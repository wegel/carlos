//! Check catalog pages against their requested session, revision and exact model interval.

use carlos_proto::ids::{SessionId, SettingsRevision};
use carlos_proto::settings::SettingsPage;
use serde_json::Value;

use crate::client_replies::parse;
use crate::ClientError;

#[derive(Clone)]
pub(crate) struct SettingsTarget {
    pub session: SessionId,
    pub expected: Option<SettingsRevision>,
    pub offset: u32,
    pub limit: usize,
}

pub(crate) fn page(target: SettingsTarget, value: Value) -> Result<SettingsPage, ClientError> {
    let page: SettingsPage = parse(value)?;
    if page.session != target.session {
        return Err(ClientError::ForeignSession);
    }
    let end = page.offset.checked_add(page.models.len() as u32);
    let valid_end = end.is_some_and(|end| {
        end <= page.settings.model_count
            && if end < page.settings.model_count {
                page.next == Some(end) && !page.models.is_empty()
            } else {
                page.next.is_none()
            }
    });
    if page.offset != target.offset
        || page.models.len() > target.limit
        || !valid_end
        || target
            .expected
            .is_some_and(|expected| expected != page.settings.revision)
    {
        return Err(ClientError::InvalidPayload(
            "model page differs from its requested revision or interval".into(),
        ));
    }
    Ok(page)
}
