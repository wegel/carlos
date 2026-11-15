//! Construct the private terminal composer without owning a backend or session.

/// Create the shared composer widget with its existing defaults.
pub(super) fn make_input_area() -> ratatui_textarea::TextArea<'static> {
    ratatui_textarea::TextArea::default()
}
