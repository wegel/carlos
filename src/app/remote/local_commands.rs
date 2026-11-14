//! Change device display choices without sending a daemon or native command.

use super::Device;
use crate::app::input::make_input_area;
use crate::app::tool_output_command::parse_tools_command;
use crate::app::tool_output_preferences::save_mode;

impl Device {
    pub(super) fn local_command(&mut self, text: &str) -> bool {
        let Some(choice) = parse_tools_command(self.view.tool_output.mode, text) else {
            return false;
        };
        match choice {
            Ok(mode) => {
                self.view.tool_output.mode = mode;
                self.view.input = make_input_area();
                if let Err(error) = save_mode(mode) {
                    self.view.status_hint = Some(format!(
                        "Tool output changed; could not save choice: {error}"
                    ));
                }
            }
            Err(message) => self.view.status_hint = Some(message.into()),
        }
        true
    }
}
