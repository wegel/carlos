//! Row layout and per-field drawing for the model-settings overlay.

use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::{content_width, OverlayBox};
use crate::app::render::draw_str;
use crate::app::state::{AppState, ModelSettingsField};
use crate::theme::{COLOR_DIM, COLOR_STEP6, COLOR_TEXT};

pub(super) fn draw_model_settings_fields(buf: &mut Buffer, ob: &OverlayBox, app: &AppState) {
    let layout = FieldLayout::new(ob);
    layout.draw_row(
        buf,
        app,
        ob.start_y + 3,
        "Model",
        8,
        ModelSettingsField::Model,
    );
    layout.draw_row(
        buf,
        app,
        ob.start_y + 5,
        "Thinking",
        8,
        ModelSettingsField::Effort,
    );
    let dictation_y = if app.runtime_supports_summary() {
        layout.draw_row(
            buf,
            app,
            ob.start_y + 7,
            "Summary",
            8,
            ModelSettingsField::Summary,
        );
        ob.start_y + 9
    } else {
        ob.start_y + 7
    };
    layout.draw_row(
        buf,
        app,
        dictation_y,
        "Dictation",
        9,
        ModelSettingsField::DictationEndpoint,
    );
    draw_str(
        buf,
        layout.x_label,
        dictation_y + 2,
        "Tab switch field, arrows adjust, Enter apply/toggle",
        Style::default().fg(COLOR_DIM),
        content_width(ob),
    );
}

struct FieldLayout {
    x_label: usize,
    x_value: usize,
    value_w: usize,
    label_style: Style,
}

impl FieldLayout {
    fn new(ob: &OverlayBox) -> Self {
        Self {
            x_label: ob.start_x + 3,
            x_value: ob.start_x + 12,
            value_w: ob.box_w.saturating_sub(16),
            label_style: Style::default().fg(COLOR_DIM),
        }
    }

    fn draw_row(
        &self,
        buf: &mut Buffer,
        app: &AppState,
        y: usize,
        label: &str,
        label_w: usize,
        field: ModelSettingsField,
    ) {
        let value = match field {
            ModelSettingsField::Model => app.model_settings_model_value(),
            ModelSettingsField::Effort => app.model_settings_effort_value(),
            ModelSettingsField::Summary => app.model_settings_summary_value(),
            ModelSettingsField::DictationEndpoint => app.dictation_endpoint_mode_label(),
        };
        draw_str(buf, self.x_label, y, label, self.label_style, label_w);
        draw_str(
            buf,
            self.x_value,
            y,
            value,
            field_style(app.runtime.model_settings_field == field),
            self.value_w,
        );
    }
}

fn field_style(active: bool) -> Style {
    if active {
        Style::default()
            .fg(COLOR_TEXT)
            .bg(COLOR_STEP6)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_TEXT)
    }
}
