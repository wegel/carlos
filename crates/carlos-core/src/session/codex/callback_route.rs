//! Route native requests and scoped control replies before root-thread filtering.

use serde_json::Value;

use super::callback_receive::CallbackReceived;
use super::callback_update::{CallbackEffect, CallbackUpdate};
use super::goals::GoalUpdate;
use super::panel_scope::{PanelScope, PanelUpdate};
use super::queue::QueueUpdate;
use super::settings::SettingsUpdate;
use crate::session::actions::ServerRequestAction;
use crate::session::approval_state::ApprovalQueue;
use crate::session::state::SessionState;

impl SessionState {
    /// Consume a native callback even during rebuild, using only explicit client targets.
    // Keep the native control router out of the shared per-frame parser.
    #[inline(never)]
    pub(crate) fn route_codex_callback(
        &mut self,
        method: &str,
        frame: &Value,
        panel: PanelScope<'_>,
    ) -> Option<CallbackUpdate> {
        if frame.get("id").is_some() {
            return Some(self.accept_codex_callback(method, frame));
        }
        let params = &frame["params"];
        let effect = match method {
            "serverRequest/resolved" => self
                .resolve_codex_callback(params)
                .map(CallbackEffect::Retired)
                .unwrap_or(CallbackEffect::None),
            "carlos/codex/formChecked" => return Some(self.checked_codex_callback(params)),
            "carlos/codex/controlReply" => return Some(self.route_codex_control(params, panel)),
            "thread/goal/updated" | "thread/goal/cleared" => {
                let update = GoalUpdate::default();
                super::goal_receive::observed(self, method, params);
                CallbackEffect::Goal(update)
            }
            "thread/queue/changed" => {
                let mut update = QueueUpdate::default();
                super::queue_receive::changed(self, &mut update, params);
                CallbackEffect::Queue(update)
            }
            "thread/settings/updated" => {
                super::settings::observed(self, params);
                CallbackEffect::None
            }
            _ => return None,
        };
        Some(CallbackUpdate::new(effect))
    }

    /// Accept a card and choose its session focus before the client creates its editor.
    pub fn accept_codex_callback(&mut self, method: &str, frame: &Value) -> CallbackUpdate {
        match self.receive_codex_callback(method, frame) {
            CallbackReceived::Ignored => CallbackUpdate::new(CallbackEffect::None),
            CallbackReceived::Rejected { id, code, message } => CallbackUpdate {
                effect: CallbackEffect::None,
                action: Some(ServerRequestAction::ReplyError {
                    request_id: id,
                    code,
                    message,
                }),
            },
            CallbackReceived::Accepted {
                key,
                card_id,
                editor,
                blocking,
                preflight,
            } => {
                let id = key.id();
                let active = self.approval().pending().is_none();
                if active {
                    self.focus_approval(card_id, ApprovalQueue::Codex);
                }
                CallbackUpdate {
                    effect: CallbackEffect::Accepted {
                        key,
                        editor,
                        card: card_id,
                        blocking,
                        active,
                    },
                    action: preflight.map(|schema| ServerRequestAction::ValidateCodexForm {
                        id,
                        schema,
                        answer: None,
                        revision: 0,
                    }),
                }
            }
        }
    }

    /// Match a form worker to its submitted candidate before returning editor changes.
    pub fn checked_codex_callback(&mut self, params: &Value) -> CallbackUpdate {
        let Some(mut update) = self.check_codex_callback(params) else {
            return CallbackUpdate::new(CallbackEffect::None);
        };
        let action = update
            .reply
            .take()
            .map(|(card_id, result)| ServerRequestAction::ReplyNative { card_id, result });
        CallbackUpdate {
            effect: CallbackEffect::Checked {
                id: params["requestId"].clone(),
                update,
            },
            action,
        }
    }

    fn route_codex_control(&mut self, params: &Value, panel: PanelScope<'_>) -> CallbackUpdate {
        if let Some(reply) = crate::session::task_stop_reply::codex(params) {
            return CallbackUpdate::new(CallbackEffect::TaskStop(reply));
        }
        if super::task_status::reply(self, panel, params)
            || super::reconnect_runtime::reply(self, params)
        {
            return CallbackUpdate::new(CallbackEffect::None);
        }
        let mut items = Default::default();
        if super::reconnect_streams::reply(self, &mut items, params) {
            return CallbackUpdate::new(CallbackEffect::Items(items));
        }
        let mut goal = GoalUpdate::default();
        let scope = self.codex_scope();
        if super::goal_receive::reply(self, scope, &mut goal, params) {
            return CallbackUpdate::new(CallbackEffect::Goal(goal));
        }
        let mut update = PanelUpdate::default();
        if super::panel_receive::reply(self, panel, &mut update, params) {
            return CallbackUpdate::new(CallbackEffect::Panel(update));
        }
        let interrupt = self.codex_callback_control_reply(params);
        let mut settings = SettingsUpdate::default();
        super::settings_reply::reply(self, scope, &mut settings, params);
        let mut queue = QueueUpdate::default();
        let action = super::queue_receive::reply(self, scope, &mut queue, params)
            .map(|(token, cursor)| ServerRequestAction::CodexQueuePage { token, cursor });
        CallbackUpdate {
            effect: CallbackEffect::Controls {
                interrupt,
                settings,
                queue,
            },
            action,
        }
    }
}
