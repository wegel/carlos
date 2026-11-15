//! Apply owner-generated control completions without charging them as received native bytes.

use super::owner::Owner;

impl Owner {
    /// Reduce generated facts in order without acknowledging bytes that never entered the native queue.
    pub(super) fn drain_control_events(&mut self) {
        let context = self.session.backend_context();
        for line in self.client.take_control_events() {
            let update = self.session.handle_backend_line(&line, &context);
            self.report(update);
        }
        if self.session.events.backend_closed {
            self.close_native();
        }
    }
}
