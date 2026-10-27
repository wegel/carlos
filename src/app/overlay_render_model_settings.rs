//! Draw settings rows and place the cursor, including on phone-sized terminals.

use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::{compute_overlay_box, content_width, OverlayBox};
use crate::app::models::TerminalSize;
use crate::app::render::draw_str;
use crate::app::state::{AppState, ModelSettingsField};
use crate::app::text::visual_width;
use crate::theme::{COLOR_DIM, COLOR_STEP6, COLOR_TEXT};

/// Fit the settings panel to the terminal, using closer rows on short screens.
pub(super) fn model_settings_overlay_box(size: TerminalSize) -> OverlayBox {
    let box_w = size.width.saturating_sub(4).min(84).max(1).min(size.width);
    let box_h = if size.height < 22 { 12 } else { 20 };
    compute_overlay_box(size, box_w, box_h.min(size.height))
}

/// Place the cursor beside the selected value without crossing the panel edge.
pub(in crate::app) fn model_settings_cursor_position(
    app: &AppState,
    size: TerminalSize,
) -> (usize, usize) {
    let ob = model_settings_overlay_box(size);
    let layout = FieldLayout::new(&ob);
    let fields = app.runtime.model_settings_fields();
    let index = fields
        .iter()
        .position(|&field| field == app.runtime.model_settings_field)
        .unwrap_or(0);
    let value_width = visual_width(field_value(app, app.runtime.model_settings_field));
    let x = (layout.x_value + value_width.min(layout.value_w)).min(ob.right.saturating_sub(1));
    let y = layout.row_y(&ob, index).min(size.height.saturating_sub(1));
    (x, y)
}

pub(super) fn draw_model_settings_fields(buf: &mut Buffer, ob: &OverlayBox, app: &AppState) {
    let layout = FieldLayout::new(ob);
    let fields = app.runtime.model_settings_fields();
    for (index, &field) in fields.iter().enumerate() {
        layout.draw_row(buf, app, layout.row_y(ob, index), field);
    }
    let help = if app.runtime.model_settings_field == ModelSettingsField::DictationEndpoint {
        "Arrows/Enter toggle, Esc close"
    } else {
        "Tab field, arrows, Enter save"
    };
    draw_str(
        buf,
        layout.x_label,
        layout.row_y(ob, fields.len()),
        help,
        Style::default().fg(COLOR_DIM),
        content_width(ob),
    );
}

struct FieldLayout {
    x_label: usize,
    x_value: usize,
    value_w: usize,
    row_step: usize,
}

impl FieldLayout {
    fn new(ob: &OverlayBox) -> Self {
        Self {
            x_label: ob.start_x + 3,
            x_value: ob.start_x + 15,
            value_w: ob.box_w.saturating_sub(18),
            row_step: if ob.bottom.saturating_sub(ob.top) + 1 < 18 {
                1
            } else {
                2
            },
        }
    }

    fn row_y(&self, ob: &OverlayBox, index: usize) -> usize {
        ob.start_y + 3 + index * self.row_step
    }

    fn draw_row(&self, buf: &mut Buffer, app: &AppState, y: usize, field: ModelSettingsField) {
        let label = match field {
            ModelSettingsField::Model => "Model",
            ModelSettingsField::Effort => "Thinking",
            ModelSettingsField::Summary => "Summary",
            ModelSettingsField::DictationEndpoint => "Dictation",
            ModelSettingsField::Layout => "Layout",
            ModelSettingsField::ToolLabels => "AI labels",
            ModelSettingsField::ToolOutput => "Tool output",
        };
        draw_str(
            buf,
            self.x_label,
            y,
            label,
            Style::default().fg(COLOR_DIM),
            11,
        );
        draw_str(
            buf,
            self.x_value,
            y,
            field_value(app, field),
            field_style(app.runtime.model_settings_field == field),
            self.value_w,
        );
    }
}

fn field_value(app: &AppState, field: ModelSettingsField) -> &str {
    match field {
        ModelSettingsField::Model => app.model_settings_model_value(),
        ModelSettingsField::Effort => app.model_settings_effort_value(),
        ModelSettingsField::Summary => app.model_settings_summary_value(),
        ModelSettingsField::DictationEndpoint => app.dictation_endpoint_mode_label(),
        ModelSettingsField::Layout => app.layout.draft_label(),
        ModelSettingsField::ToolLabels => {
            if app.summaries.draft {
                "On"
            } else {
                "Off"
            }
        }
        ModelSettingsField::ToolOutput => app
            .tool_output
            .settings_label(app.runtime.model_settings_tool_output),
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
