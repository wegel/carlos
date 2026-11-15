//! Drive private native windows through the existing request tokens, bounded parser and deadlines.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use carlos_proto::ids::TaskWindowId;
use carlos_proto::task_windows::MAX_NATIVE_TASK_BYTES;
use serde_json::Value;

use super::codex::panel;
use super::codex::panel_reader::PageStep;
use super::codex::panel_state::{Request, Snapshot, VerifiedOwners};
use super::state::SessionState;
use super::task_window_state::{Captured, Contents, Reader, Scope};
use super::task_windows::window_revision;

impl SessionState {
    pub(in crate::session) fn receive_task_window(&mut self, params: &Value) -> bool {
        let Some(token) = params["requestContext"]["token"].as_u64() else {
            return false;
        };
        let Some((id, request)) = self.task_windows.request(token) else {
            return false;
        };
        if params["requestMethod"] != request.method || params["requestParams"] != request.params {
            return true;
        }
        let (id, request) = (id.clone(), request.clone());
        let error = params["response"].get("error").map(|error| {
            error["message"]
                .as_str()
                .unwrap_or("native task window read failed")
                .to_string()
        });
        self.apply_task_window(
            &id,
            &request,
            &params["response"]["result"],
            error.as_deref(),
        );
        true
    }

    pub(in crate::session) fn fail_task_window(&mut self, request: &Request, error: &str) -> bool {
        let Some((id, _)) = self.task_windows.request(request.token) else {
            return false;
        };
        let id = id.clone();
        self.apply_task_window(&id, request, &Value::Null, Some(error));
        true
    }

    fn apply_task_window(
        &mut self,
        id: &TaskWindowId,
        previous: &Request,
        result: &Value,
        error: Option<&str>,
    ) {
        let mut slot = self.task_windows.slots.remove(id).expect("matched window");
        if !slot.reader.load(Ordering::Acquire) || slot.scope != Scope::capture(self) {
            return;
        }
        let Contents::Reading(mut read) = slot.contents else {
            return;
        };
        let budget = self.task_window_bytes().saturating_add(read.bytes);
        let step = if budget.saturating_add(super::task_window_bytes::encoded_bytes(result))
            > MAX_NATIVE_TASK_BYTES
        {
            Err(
                "Native task windows exceed their shared 8 MiB allowance; previous page retained"
                    .into(),
            )
        } else if let Some(error) = error {
            if previous.method == "thread/backgroundTerminals/list" {
                Ok(read.process_failed(error))
            } else {
                Err(error.to_string())
            }
        } else {
            read.page(result)
        };
        slot.contents = match step {
            Ok(PageStep::Continue(cursor)) => {
                read.request = panel::continue_read(self, previous.clone(), cursor);
                Contents::Reading(read)
            }
            Ok(PageStep::Read { method, params }) => {
                read.request = panel::request(self, method, params);
                Contents::Reading(read)
            }
            Ok(PageStep::Complete) => self.capture_task_window(id, read),
            Err(error) => Contents::Failed(short_error(&error)),
        };
        self.task_windows.slots.insert(id.clone(), slot);
    }

    fn capture_task_window(&mut self, id: &TaskWindowId, read: Snapshot) -> Contents {
        if read.revision != self.codex_panel.revision {
            return Contents::Failed("Native tasks changed while reading the older window; previous page retained. l explicitly retries".into());
        }
        let owners = match VerifiedOwners::new(
            &read.root,
            self.codex_scope().epoch(),
            read.verified_owners(),
        ) {
            Ok(owners) => owners,
            Err(error) => return Contents::Failed(error),
        };
        let bytes = self.task_snapshot_bytes(&read);
        if self.task_window_bytes().saturating_add(bytes) > MAX_NATIVE_TASK_BYTES {
            return Contents::Failed(
                "Native task windows exceed their shared 8 MiB allowance; previous page retained"
                    .into(),
            );
        }
        Contents::Ready(Captured {
            read_error: (!read.errors.is_empty()).then(|| short_error(&read.errors.join("\n"))),
            agents: read.agents,
            processes: read.processes,
            owners,
            process_scopes: read.process_scopes,
            reports: self.codex_panel.reports.clone(),
            older: read.older,
            bytes,
            revision: window_revision(id),
        })
    }

    pub(in crate::session) fn expire_task_windows(&mut self) {
        let tokens: Vec<_> = self
            .task_windows
            .slots
            .values()
            .filter_map(|slot| {
                if let Contents::Reading(read) = &slot.contents {
                    Some(read.request.token)
                } else {
                    None
                }
            })
            .collect();
        let scope = Scope::capture(self);
        let dead: Vec<_> = self
            .task_windows
            .slots
            .values()
            .filter(|slot| !slot.reader.load(Ordering::Acquire) || slot.scope != scope)
            .map(|slot| slot.reader.clone())
            .collect();
        for reader in dead {
            self.retire_task_windows(&reader, None);
        }
        self.task_windows.slots.retain(|_, slot| {
            let live = slot.reader.load(Ordering::Acquire) && slot.scope == scope;
            match &slot.contents {
                Contents::Reading(read) if read.started.elapsed() >= Duration::from_secs(20) => {
                    slot.contents = Contents::Failed(
                        "Older task window exceeded 20 seconds; previous page retained".into(),
                    );
                    live
                }
                Contents::Retired { started, .. } => started.elapsed() < Duration::from_secs(20),
                _ => live,
            }
        });
        self.prune_task_window_requests();
        self.codex_panel.outgoing.retain(|request| !tokens.contains(&request.token)
            || self.task_windows.request(request.token).is_some_and(|(_, request)| {
                self.task_windows.slots.values().any(|slot| matches!(&slot.contents, Contents::Reading(read) if read.request.token == request.token))
            }));
    }

    pub(in crate::session) fn retire_task_windows(
        &mut self,
        reader: &Reader,
        id: Option<&TaskWindowId>,
    ) {
        let queued: Vec<_> = self
            .codex_panel
            .outgoing
            .iter()
            .map(|request| request.token)
            .collect();
        let mut removed = Vec::new();
        self.task_windows.slots.retain(|key, slot| {
            if !Arc::ptr_eq(reader, &slot.reader) || id.is_some_and(|id| id != key) {
                return true;
            }
            if matches!(slot.contents, Contents::Retired { .. }) {
                return true;
            }
            if let Contents::Reading(read) = &slot.contents {
                removed.push(read.request.token);
                if !queued.contains(&read.request.token) {
                    slot.contents = Contents::Retired {
                        request: read.request.clone(),
                        started: read.started,
                    };
                    return true;
                }
            }
            false
        });
        self.codex_panel
            .outgoing
            .retain(|request| !removed.contains(&request.token));
    }

    pub(in crate::session) fn prune_task_window_requests(&mut self) {
        // Other panel consumers own their own tokens. Only remove retired window tokens here.
        let retired: Vec<_> = self
            .task_windows
            .slots
            .values()
            .filter_map(|slot| {
                if let Contents::Retired { request, .. } = &slot.contents {
                    Some(request.token)
                } else {
                    None
                }
            })
            .collect();
        self.codex_panel
            .outgoing
            .retain(|request| !retired.contains(&request.token));
    }
}

fn short_error(message: &str) -> String {
    super::text::safe_text(&message.chars().take(1024).collect::<String>())
}
