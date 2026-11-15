//! Retain one bounded terminal cause while an earlier successful result awaits its interrupt reply.

use carlos_proto::submissions::ClientOperation;
use serde_json::{json, Value};

use super::event_normalizer::fact;
use crate::protocol_events::bounded_diagnostic;

/// Retain known diagnostic fields without holding an arbitrary native payload.
pub(super) fn bounded_terminal(
    method: &str,
    params: &Value,
    generation: u64,
    epoch: u64,
) -> String {
    let mut retained = json!({"generation":generation,"epoch":epoch});
    for name in ["message", "reason", "text", "requestMethod"] {
        if let Some(value) = params[name].as_str() {
            retained[name] = json!(bounded_diagnostic(value));
        }
    }
    if let Some(error) = params.get("error").filter(|error| error.is_object()) {
        retained["error"] = json!({
            "message":bounded_diagnostic(error["message"].as_str().unwrap_or("Claude helper closed"))
        });
        if let Some(code) = error["code"].as_str() {
            retained["error"]["code"] = json!(bounded_diagnostic(code));
        }
    }
    if params.get("ok").is_some() {
        retained["ok"] = json!(params["ok"] == true);
    }
    if let Some(operation) = params
        .get("operation")
        .and_then(|value| serde_json::from_value::<ClientOperation>(value.clone()).ok())
    {
        retained["operation"] = json!(operation);
    }
    fact(method, retained)
}
