//! Fence native generation envelopes and return one synchronous drain to the client.

use serde_json::{json, Map, Value};

use super::frame_batch::NativeBatch;
use crate::backend::BackendClient;
use crate::session::backend_update::{BackendChange, BackendContext, BackendUpdate};
use crate::session::state::SessionState;

impl SessionState {
    /// Accept one wrapped frame, retaining charges until its reports and replies are consumed.
    pub fn receive_native_line(
        &mut self,
        client: &dyn BackendClient,
        line: &str,
        context: &BackendContext<'_>,
    ) -> Option<NativeBatch> {
        let frame: Value = serde_json::from_str(line).ok()?;
        if frame["method"] != "carlos/codex/frame" {
            return None;
        }
        let mut batch = NativeBatch::new(line.len());
        let params = &frame["params"];
        if params["generation"].as_u64() != Some(self.codex_connection().generation()) {
            return Some(batch);
        }
        let event = &params["event"];
        let method = event["method"].as_str().unwrap_or("");
        if method == "carlos/codex/closed" {
            self.drain_codex_replay(client);
            self.stop_codex_rebuild();
            batch.initial = Some(self.native_frame_closed(&event["params"], ""));
        } else if method == "carlos/codex/ready" && self.codex_connection().rebuilding() {
            return Some(self.begin_native_ready(client, line.len()));
        } else if self.codex_connection().rebuilding() && !callback(event) {
            if self.hold_codex_replay(event.clone(), line.len()) {
                batch.root_charge = None;
                return Some(batch);
            }
            self.drain_codex_replay(client);
            self.stop_codex_rebuild();
            batch.initial = Some(self.native_frame_closed(
                &json!({"message":"Codex reconnect replay exceeds12 MiB or2048 frames"}),
                "",
            ));
            batch.abort = true;
        } else if self.codex_connection().rebuilding() {
            batch.initial = self
                .route_codex_callback(method, event, context.panel)
                .map(|update| BackendUpdate::new(BackendChange::Codex(update)));
        } else if !self.codex_scope().closed() {
            batch.initial = Some(self.apply_native_frame(event, context));
        }
        Some(batch)
    }

    /// Accept partial-item reads before shared dispatch, preserving their original scope.
    pub(in crate::session) fn apply_native_frame(
        &mut self,
        event: &Value,
        context: &BackendContext<'_>,
    ) -> BackendUpdate {
        let mut changes = Default::default();
        if super::reconnect_streams::event(self, &mut changes, event) {
            BackendUpdate::new(BackendChange::Items(changes))
        } else {
            self.handle_backend_message(event, context)
        }
    }

    /// Close wrapped native failures with the original permissive parameter default.
    pub(in crate::session) fn native_frame_closed(
        &mut self,
        params: &Value,
        guidance: &str,
    ) -> BackendUpdate {
        let empty = Map::new();
        let change = self
            .close_codex_native(params.as_object().unwrap_or(&empty), guidance)
            .map(BackendChange::Closed)
            .unwrap_or(BackendChange::None);
        BackendUpdate::new(change)
    }
}

fn callback(event: &Value) -> bool {
    event.get("method").is_some() && event.get("id").is_some()
        || matches!(
            event["method"].as_str(),
            Some("serverRequest/resolved" | "carlos/codex/formChecked")
        )
        || event["method"] == "carlos/codex/controlReply"
            && event["params"]["requestMethod"] == "turn/interrupt"
}
