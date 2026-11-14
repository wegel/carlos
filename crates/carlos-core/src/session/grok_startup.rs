//! Mark Grok ready only after its native replay tail has reached the session owner.

use serde_json::{json, Value};

use super::backend_update::{BackendChange, BackendUpdate};
use super::common_update::CommonUpdate;
use super::models::Role;
use super::state::SessionState;

impl SessionState {
    /// Keep human prompts queued while a native session opens and replays its saved rows.
    pub fn begin_grok_start(&mut self, restoring: bool) {
        self.events.launch = Some(if restoring {
            carlos_proto::session::Availability::Restoring
        } else {
            carlos_proto::session::Availability::Starting
        });
        self.set_status("Grok starting");
    }

    pub(in crate::session) fn handle_grok_lifecycle(
        &mut self,
        method: &str,
        params: &Value,
    ) -> BackendUpdate {
        if method == "carlos/grok/closed" {
            self.events.backend_closed = true;
            self.retire_backend_command();
            self.set_active_turn_id(None);
            self.ralph_stop_after_failure();
            self.append_message(
                Role::System,
                format!(
                    "Grok closed: {}",
                    super::text::safe_text(
                        params["message"].as_str().unwrap_or("native stream closed")
                    )
                ),
            );
            return BackendUpdate::new(BackendChange::None);
        }
        let Some(id) = params["threadId"].as_str().filter(|id| !id.is_empty()) else {
            return BackendUpdate::new(BackendChange::None);
        };
        let Ok((models, _)) = crate::protocol_params::parse_model_list_page(
            &json!({"result":{"data":params["models"]}}).to_string(),
        ) else {
            return BackendUpdate::new(BackendChange::None);
        };
        let thread = self.adopt_native_thread(id.into());
        self.set_available_models(models);
        self.events.launch = None;
        self.set_status("Grok ready");
        BackendUpdate::new(BackendChange::Common(CommonUpdate {
            thread: Some(thread),
            ..Default::default()
        }))
    }
}
