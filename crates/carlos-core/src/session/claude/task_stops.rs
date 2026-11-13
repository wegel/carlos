//! Retains bounded stop requests independently of evictable task detail rows.

use std::collections::BTreeMap;

/// Tracks control progress without treating an acknowledgement as a terminal task edge.
#[derive(Debug)]
pub enum StopState {
    Sending,
    Requested,
    Failed(String),
}

/// Keeps one control state per actual task/run, including level-only tasks.
#[derive(Default)]
#[cfg_attr(any(test, feature = "test-support"), derive(Debug))]
pub struct TaskStops {
    entries: BTreeMap<String, StopRequest>,
}
#[cfg_attr(any(test, feature = "test-support"), derive(Debug))]
struct StopRequest {
    run: Option<String>,
    state: StopState,
}

impl TaskStops {
    /// Start once per run; failed requests can be retried explicitly by the user.
    pub(in crate::session) fn begin(
        &mut self,
        id: &str,
        run: Option<&str>,
    ) -> Result<bool, &'static str> {
        if matches!(
            self.state(id, run),
            Some(StopState::Sending | StopState::Requested)
        ) {
            return Ok(false);
        }
        if self.entries.len() >= 4096 && !self.entries.contains_key(id) {
            return Err("Too many unanswered native task stops; wait for native task events");
        }
        self.entries.insert(
            id.to_string(),
            StopRequest {
                run: run.map(str::to_string),
                state: StopState::Sending,
            },
        );
        Ok(true)
    }

    /// Show a control state only for the exact run it belongs to.
    pub fn state(&self, id: &str, run: Option<&str>) -> Option<&StopState> {
        self.entries
            .get(id)
            .filter(|request| request.run.as_deref() == run)
            .map(|request| &request.state)
    }

    /// Keep stop acknowledgements independent of the detailed task cache.
    pub(in crate::session) fn reply(&mut self, id: &str, run: Option<&str>, error: Option<String>) {
        if let Some(request) = self
            .entries
            .get_mut(id)
            .filter(|request| request.run.as_deref() == run)
        {
            request.state = error
                .map(|error| StopState::Failed(error.chars().take(1024).collect()))
                .unwrap_or(StopState::Requested);
        }
    }

    /// Retire a control after native Claude reports that its exact task run ended.
    pub(in crate::session) fn finish(&mut self, id: &str, run: Option<&str>) {
        if self
            .entries
            .get(id)
            .is_some_and(|request| request.run.as_deref() == run)
        {
            self.entries.remove(id);
        }
    }
}
