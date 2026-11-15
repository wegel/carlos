//! Normalize fixed native frames and interrupt intent for retained transcript tests.

use serde_json::json;

/// Owns native fixture cursors independently of any app or terminal widget.
#[derive(Default)]
pub struct FixtureNormalizer {
    reader: super::event_normalizer::EventNormalizer,
    handler: super::event_handler::EventHandler,
}

impl FixtureNormalizer {
    /// Decode legacy test inputs at the same backend boundary as controller stdout.
    pub fn lines(
        &mut self,
        parsed: &serde_json::Value,
        session: &str,
        generation: u64,
    ) -> Vec<String> {
        if parsed["method"] != "claude/controller" {
            return self.handler.handle(&parsed.to_string());
        }
        let params = &parsed["params"];
        self.reader.history.session = Some(session.to_string());
        let lines = self
            .reader
            .route(
                &params["frame"],
                params["requestMethod"].as_str(),
                &params["requestContext"],
                params["epoch"].as_u64().unwrap_or(0),
            )
            .expect("normalize native fixture")
            .into_iter()
            .map(|line| {
                let mut value: serde_json::Value =
                    serde_json::from_str(&line).expect("normalized JSON");
                value["params"]["generation"] = json!(generation);
                value.to_string()
            })
            .collect::<Vec<_>>();
        lines
            .iter()
            .flat_map(|line| self.handler.handle(line))
            .collect()
    }

    /// Capture the native interrupt intent in tests that previously reached into the app.
    pub fn begin_interrupt(&mut self) -> Vec<String> {
        let captured = self.handler.lifecycle.begin_interrupt();
        self.handler.lifecycle.finish_interrupt(&captured, true);
        captured
    }

    /// Undo an interrupt that a fixture failed to send.
    pub fn rollback_interrupt(&mut self, ids: &[String]) {
        self.handler.lifecycle.rollback_interrupt(ids);
    }
}
