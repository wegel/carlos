//! Draw settings rows and place the cursor, including on phone-sized terminals.

use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::{compute_overlay_box, content_width, OverlayBox};

use crate::app::render::draw_str;
use crate::app::runtime_settings_state::ModelSettingsField;

use crate::app::text::visual_width;
use crate::app::view_models::TerminalSize;
use crate::theme::{COLOR_DIM, COLOR_STEP6, COLOR_TEXT};

/// Fit the settings panel to the terminal, using closer rows on short screens.
pub(super) fn model_settings_overlay_box(size: TerminalSize) -> OverlayBox {
    let box_w = size.width.saturating_sub(4).clamp(1, 84).min(size.width);
    let box_h = 26;
    compute_overlay_box(size, box_w, box_h.min(size.height))
}

/// Borrow display values without granting the painter access to a session or editor mutation.
pub(in crate::app) struct SettingsFrame<'a> {
    pub fields: Vec<(ModelSettingsField, &'a str)>,
    pub selected: ModelSettingsField,
    pub help: &'a str,
}

/// Place the cursor beside the selected value without crossing the panel edge.
pub(in crate::app) fn settings_cursor_position(
    frame: &SettingsFrame<'_>,
    size: TerminalSize,
) -> (usize, usize) {
    let ob = model_settings_overlay_box(size);
    let layout = FieldLayout::new(&ob, frame);
    let index = frame
        .fields
        .iter()
        .position(|(field, _)| *field == frame.selected)
        .unwrap_or(0);
    let value = frame.fields.get(index).map_or("", |(_, value)| *value);
    let value_width = visual_width(value);
    let x = (layout.x_value + value_width.min(layout.value_w)).min(ob.right.saturating_sub(1));
    let y = layout.row_y(&ob, index).min(size.height.saturating_sub(1));
    (x, y)
}

pub(super) fn draw_model_settings_fields(
    buf: &mut Buffer,
    ob: &OverlayBox,
    frame: &SettingsFrame<'_>,
) {
    let layout = FieldLayout::new(ob, frame);
    for (index, &(field, value)) in frame
        .fields
        .iter()
        .enumerate()
        .skip(layout.first)
        .take(layout.rows)
    {
        layout.draw_row(
            buf,
            layout.row_y(ob, index),
            field,
            value,
            frame.selected == field,
        );
    }
    draw_str(
        buf,
        layout.x_label,
        ob.bottom.saturating_sub(1),
        frame.help,
        Style::default().fg(COLOR_DIM),
        content_width(ob),
    );
}

struct FieldLayout {
    x_label: usize,
    x_value: usize,
    value_w: usize,
    row_step: usize,
    padding: usize,
    first: usize,
    rows: usize,
}

impl FieldLayout {
    fn new(ob: &OverlayBox, frame: &SettingsFrame<'_>) -> Self {
        let fields = &frame.fields;
        let height = ob.bottom.saturating_sub(ob.top) + 1;
        let padding = if height < 13 { 2 } else { 3 };
        let row_step = if height >= fields.len() * 2 + padding + 3 {
            2
        } else {
            1
        };
        let rows = (height.saturating_sub(padding + 3) / row_step).max(1);
        let selected = fields
            .iter()
            .position(|(field, _)| *field == frame.selected)
            .unwrap_or(0);
        Self {
            x_label: ob.start_x + 3,
            x_value: ob.start_x + 15,
            value_w: ob.box_w.saturating_sub(18),
            row_step,
            padding,
            first: selected.saturating_sub(rows.saturating_sub(1)),
            rows,
        }
    }

    fn row_y(&self, ob: &OverlayBox, index: usize) -> usize {
        ob.start_y + self.padding + index.saturating_sub(self.first) * self.row_step
    }

    fn draw_row(
        &self,
        buf: &mut Buffer,
        y: usize,
        field: ModelSettingsField,
        value: &str,
        active: bool,
    ) {
        let label = match field {
            ModelSettingsField::Model => "Model",
            ModelSettingsField::Effort => "Thinking",
            ModelSettingsField::Summary => "Summary",
            ModelSettingsField::Context => "Context",
            ModelSettingsField::DictationEndpoint => "Dictation",
            ModelSettingsField::DesktopTheme => "Desktop",
            ModelSettingsField::PhoneTheme => "Phone",
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
            value,
            field_style(active),
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
