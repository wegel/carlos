//! Resolve typed Claude sources before accepting or replacing printable cursor aliases.

use crate::app::state::AppState;
use crate::app::Role;
use crate::session::entry_key::{BackendEntryKey, NativeEntryKey};

impl AppState {
    /// Bind a new translator cursor to native work that Carlos already knows.
    pub(in crate::app) fn prepare_item_source(
        &mut self,
        item: &str,
        source: Option<&serde_json::Value>,
        all_parts: bool,
    ) {
        let Some(mut source) = self.native_source_key(item, source) else {
            self.reject_unproven_cursor(item, all_parts);
            return;
        };
        let mut live = self.item_key(item);
        let count = if all_parts {
            self.messages
                .part_count(&source)
                .max(self.messages.part_count(&live))
                .max(1)
        } else {
            1
        };
        for _ in 0..count {
            self.prepare_cursor(&live, &source);
            source.part += 1;
            live.part += 1;
        }
    }

    /// Keep Claude's native evidence separate from generated live correlation counters.
    pub(in crate::app) fn attach_item_source(
        &mut self,
        item: &str,
        source: Option<&serde_json::Value>,
        all_parts: bool,
    ) {
        let Some(mut source) = self.native_source_key(item, source) else {
            return;
        };
        let mut live = self.item_key(item);
        while let Some(id) = self.messages.entry_id_for_item(&live) {
            self.messages.bind_item(source.clone(), id);
            source.part += 1;
            live.part += 1;
            if !all_parts {
                break;
            }
        }
    }

    fn native_source_key(
        &self,
        item: &str,
        source: Option<&serde_json::Value>,
    ) -> Option<BackendEntryKey> {
        let source = source?;
        let native = source["id"].as_str().filter(|id| !id.is_empty())?;
        let mut key = self.item_key(item);
        key.item = match source["kind"].as_str() {
            Some("assistant") => NativeEntryKey::ClaudeAssistant {
                id: native.to_string(),
                block: source["block"].as_u64()?,
            },
            Some("tool") => NativeEntryKey::ClaudeTool {
                id: native.to_string(),
                result: match source["part"].as_str() {
                    Some("result") => true,
                    Some("call") => false,
                    _ => return None,
                },
            },
            Some("user") => NativeEntryKey::ClaudePrompt(native.to_string()),
            _ => return None,
        };
        if let Some(session) = source["sessionId"].as_str().filter(|id| !id.is_empty()) {
            key.conversation = session.to_string();
        }
        Some(key)
    }

    fn prepare_cursor(&mut self, live: &BackendEntryKey, source: &BackendEntryKey) {
        if let Some(id) = self.messages.entry_id_for_item(&source) {
            for retired in self.messages.retire_cursors(id, &live) {
                self.completed_item_ids.remove(&retired);
            }
            self.completed_item_ids.remove(&live);
            if self.messages.is_complete(id) {
                self.completed_item_ids.insert(live.clone());
            }
            self.messages.bind_item(live.clone(), id);
        } else if self.cursor_conflicts(live, source) {
            self.messages.unbind_item(&live);
            self.completed_item_ids.remove(&live);
        }
    }

    fn reject_unproven_cursor(&mut self, item: &str, all_parts: bool) {
        if !self.claude.enabled {
            return;
        }
        let mut live = self.item_key(item);
        let count = if all_parts {
            self.messages.part_count(&live).max(1)
        } else {
            1
        };
        for _ in 0..count {
            if self
                .messages
                .entry_id_for_item(&live)
                .and_then(|id| self.messages.source(id))
                .is_some()
            {
                self.messages.unbind_item(&live);
                self.completed_item_ids.remove(&live);
            }
            live.part += 1;
        }
    }

    fn cursor_conflicts(&self, live: &BackendEntryKey, source: &BackendEntryKey) -> bool {
        self.messages.entry_id_for_item(live).is_some_and(|id| {
            self.messages.source(id).map_or_else(
                || {
                    let role = self.messages.entry(id).expect("live cursor").role;
                    match source.item {
                        NativeEntryKey::ClaudeTool { result: false, .. } => role != Role::ToolCall,
                        NativeEntryKey::ClaudeTool { result: true, .. } => role != Role::ToolOutput,
                        NativeEntryKey::ClaudeAssistant { .. } => {
                            !matches!(role, Role::Assistant | Role::Commentary | Role::Reasoning)
                        }
                        _ => false,
                    }
                },
                |existing| existing != source,
            )
        })
    }
}
