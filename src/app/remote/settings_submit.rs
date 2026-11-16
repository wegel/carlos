//! Send each captured Save step once, retaining the original native revision throughout.

use anyhow::Result;

use super::settings_intent::Write;
use super::transport::Transport;
use super::Device;

impl Device {
    pub(super) fn drive_settings_save(&mut self, link: &mut Transport) -> Result<bool> {
        if self.settings.operation.is_some()
            || self.settings.appearance_save.is_some()
            || self.settings.submitted.is_none()
            || !self.client.session_is_ready(link.connection, &self.session)
        {
            return Ok(false);
        }
        let mut intent = self
            .settings
            .submitted
            .take()
            .expect("captured settings intent");
        let facts = self
            .client
            .session(&self.session)
            .and_then(|session| session.settings())
            .expect("ready settings facts");
        let command = intent.take_next(facts);
        self.settings.submitted = Some(intent);
        if let Some(command) = command {
            self.submit_settings_step(link, command)?;
        } else {
            self.finish_settings_save();
        }
        Ok(true)
    }

    fn submit_settings_step(&mut self, link: &mut Transport, command: Write) -> Result<()> {
        let mut allocated = None;
        let session = self.session.clone();
        let outgoing = match self.client.submit(link.connection, |operation| {
            allocated = Some(operation.clone());
            command.command(session, operation)
        }) {
            Ok(outgoing) => outgoing,
            Err(error) => {
                self.settings.submitted = None;
                self.settings.error = Some(format!("Settings were not sent: {error}"));
                return Ok(());
            }
        };
        self.settings.operation = allocated;
        self.settings.error = Some("Settings save pending; waiting for its original reply".into());
        let id = outgoing.id();
        if let Err(error) = link.send(outgoing) {
            let effects = self
                .client
                .write_failed(link.connection, id, &error.to_string(), false);
            self.apply_operations(&effects.operations);
            return Err(error);
        }
        Ok(())
    }
}
