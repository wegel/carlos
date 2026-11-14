//! Verify descendant parent chains against the native root captured when their read began.

use std::collections::HashMap;

use serde_json::Value;

use super::panel_state::MAX_AGENTS;

/// Read a child thread's native parent and reject conflicting parent fields.
pub fn parent(thread: &Value) -> Option<String> {
    thread["source"].get("subAgent")?;
    let source = thread["source"]["subAgent"]["thread_spawn"]["parent_thread_id"].as_str();
    let direct = thread["parentThreadId"].as_str();
    if source
        .zip(direct)
        .is_some_and(|(left, right)| left != right)
    {
        return None;
    }
    direct
        .or(source)
        .filter(|id| !id.is_empty() && id.len() <= 4096)
        .map(str::to_string)
}

/// Walk native parent ids up to the captured root with a fixed step limit.
pub(super) fn belongs(id: &str, root: &str, parents: &HashMap<String, Option<String>>) -> bool {
    let mut current = id;
    for _ in 0..MAX_AGENTS {
        let Some(Some(parent)) = parents.get(current) else {
            return false;
        };
        if parent == root {
            return true;
        }
        if parent == id {
            return false;
        }
        current = parent;
    }
    false
}
