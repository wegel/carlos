//! Parse bounded native task pages without borrowing a session or scheduling backend work.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::panel_lineage::belongs;
use super::panel_state::{Process, Request, Snapshot, MAX_AGENTS, MAX_BYTES, MAX_PROCESSES};

/// Let the caller retain its native request token, allocate a new one, or publish a complete read.
pub(super) enum PageStep {
    Continue(Value),
    Read { method: &'static str, params: Value },
    Complete,
}

impl Snapshot {
    /// Capture one root and bounded window before the caller queues its first native request.
    pub(super) fn new(root: &str, revision: u64, cursor: Value, older_window: bool) -> Self {
        Self {
            root: root.into(),
            request: Request {
                token: 0,
                method: "thread/list".into(),
                params: json!({"ancestorThreadId":root,
                    "sourceKinds":["subAgent","subAgentReview","subAgentCompact","subAgentThreadSpawn","subAgentOther"],
                    "sortKey":"updated_at","sortDirection":"desc","limit":1,"cursor":cursor}),
            },
            started: Instant::now(),
            agents: Vec::new(),
            processes: Vec::new(),
            scopes: VecDeque::new(),
            bytes: 0,
            cursors: HashSet::new(),
            revision,
            older: Value::Null,
            lineage: HashMap::new(),
            checked: HashSet::new(),
            errors: Vec::new(),
            process_scopes: HashMap::new(),
            older_window,
        }
    }

    /// Keep the last complete window usable when a malformed or oversized native page fails.
    pub(super) fn page(&mut self, result: &Value) -> Result<PageStep, String> {
        self.bytes = self.bytes.saturating_add(result.to_string().len());
        if self.bytes > MAX_BYTES || self.started.elapsed() > Duration::from_secs(20) {
            return Err("Panel read exceeds 8 MiB or 20 seconds; previous rows retained".into());
        }
        match self.request.method.as_str() {
            "thread/list" => self.agents_page(result),
            "thread/read" => self.ancestor_page(result),
            "thread/backgroundTerminals/list" => self.processes_page(result),
            _ => Err("Unexpected panel page method".into()),
        }
    }

    /// Retain an unavailable process scope without treating it as a successful empty list.
    pub(super) fn process_failed(&mut self, message: &str) -> PageStep {
        let short: String = crate::session::text::safe_text(message)
            .chars()
            .take(256)
            .collect();
        if let Some(owner) = self.request.params["threadId"].as_str() {
            self.process_scopes
                .insert(owner.to_string(), Some(short.clone()));
        }
        self.errors.push(format!(
            "Processes unavailable for {}: {short}",
            self.request.params["threadId"]
        ));
        self.next_scope()
    }

    /// Export only ids whose native parent chains reach the root captured at the start.
    pub(super) fn verified_owners(&self) -> impl Iterator<Item = String> + '_ {
        self.lineage
            .keys()
            .filter(|id| belongs(id, &self.root, &self.lineage))
            .cloned()
    }

    /// Describe whether this completed window includes every verified descendant.
    pub(super) fn notice(&self) -> &'static str {
        if self.older.is_null() && !self.older_window {
            "All verified descendants in this snapshot"
        } else {
            "Bounded agent window; l older, r latest"
        }
    }

    fn agents_page(&mut self, result: &Value) -> Result<PageStep, String> {
        let data = result["data"]
            .as_array()
            .ok_or("Malformed descendant page")?;
        for agent in data {
            let id = valid_id(&agent["id"])?;
            if self.agents.len() >= MAX_AGENTS {
                return Err("Too many agents in one native page".into());
            }
            if id == self.root {
                continue;
            }
            if self.lineage.contains_key(id) {
                return Err("Duplicate/root thread in descendant page; refresh required".into());
            }
            self.lineage
                .insert(id.into(), super::panel_lineage::parent(agent));
            self.agents.push(agent.clone());
        }
        let cursor = cursor(result)?;
        if !cursor.is_null() && self.agents.len() < MAX_AGENTS {
            self.continue_page(cursor)
        } else {
            self.older = cursor;
            self.verify()
        }
    }

    fn ancestor_page(&mut self, result: &Value) -> Result<PageStep, String> {
        let expected = valid_id(&self.request.params["threadId"])?;
        let thread = &result["thread"];
        if thread["id"] != expected {
            return Err("Ancestor read returned a different thread".into());
        }
        self.lineage
            .insert(expected.into(), super::panel_lineage::parent(thread));
        self.verify()
    }

    fn verify(&mut self) -> Result<PageStep, String> {
        let missing = self
            .lineage
            .values()
            .flatten()
            .find(|id| **id != self.root && !self.lineage.contains_key(*id))
            .cloned();
        if let Some(id) = missing {
            if self.checked.len() >= MAX_AGENTS {
                return Err("Parent chain exceeds panel bounds; relationship unavailable".into());
            }
            self.checked.insert(id.clone());
            return Ok(PageStep::Read {
                method: "thread/read",
                params: json!({"threadId":id,"includeTurns":false}),
            });
        }
        self.agents.retain(|agent| {
            belongs(
                agent["id"].as_str().unwrap_or(""),
                &self.root,
                &self.lineage,
            )
        });
        self.scopes.push_back(self.root.clone());
        for agent in &self.agents {
            if matches!(
                agent["status"]["type"].as_str(),
                Some("active" | "idle" | "systemError")
            ) {
                self.scopes.push_back(valid_id(&agent["id"])?.into());
            }
        }
        Ok(self.next_scope())
    }

    fn processes_page(&mut self, result: &Value) -> Result<PageStep, String> {
        let data = result["data"]
            .as_array()
            .ok_or("Malformed background process page")?;
        let thread = valid_id(&self.request.params["threadId"])?.to_string();
        for process in data {
            let id = valid_id(&process["processId"])?;
            if self.processes.len() >= MAX_PROCESSES {
                return Err("Background process list exceeds 512 records".into());
            }
            if self
                .processes
                .iter()
                .any(|row| row.thread == thread && row.data["processId"] == id)
            {
                return Err("Duplicate background process across pages; refresh required".into());
            }
            self.processes.push(Process {
                thread: thread.clone(),
                data: process.clone(),
            });
        }
        let cursor = cursor(result)?;
        if !cursor.is_null() {
            self.continue_page(cursor)
        } else {
            self.process_scopes.insert(thread, None);
            Ok(self.next_scope())
        }
    }

    fn continue_page(&mut self, cursor: Value) -> Result<PageStep, String> {
        let key = cursor.as_str().ok_or("Invalid panel cursor")?;
        if !self.cursors.insert(key.into()) {
            return Err("Repeated panel cursor".into());
        }
        Ok(PageStep::Continue(cursor))
    }

    fn next_scope(&mut self) -> PageStep {
        if let Some(thread) = self.scopes.pop_front() {
            self.cursors.clear();
            PageStep::Read {
                method: "thread/backgroundTerminals/list",
                params: json!({"threadId":thread,"limit":1}),
            }
        } else {
            PageStep::Complete
        }
    }
}

/// Accept bounded nonempty native IDs without deriving identity from labels.
pub(crate) fn valid_id(value: &Value) -> Result<&str, String> {
    value
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 4096)
        .ok_or_else(|| "Missing or oversized native panel ID".into())
}

/// Accept only opaque string cursors or an explicit end of pages.
pub fn cursor(result: &Value) -> Result<Value, String> {
    match result.get("nextCursor") {
        None | Some(Value::Null) => Ok(Value::Null),
        Some(value) => {
            valid_id(value)?;
            Ok(value.clone())
        }
    }
}
