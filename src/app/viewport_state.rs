use super::selection::{MouseDragMode, Selection};

pub(super) struct ViewportState {
    pub(super) scroll_top: usize,
    pub(super) auto_follow_bottom: bool,
    pub(super) selection: Option<Selection>,
    pub(super) mouse_drag_mode: MouseDragMode,
    pub(super) mouse_drag_last_row: usize,
    pub(super) mobile_mouse_buffer: String,
    pub(super) mobile_mouse_last_y: Option<usize>,
    pub(super) mobile_plain_pending_coords: bool,
    pub(super) mobile_plain_suppress_coords: bool,
    pub(super) mobile_plain_last_direction: i8,
    pub(super) mobile_plain_new_gesture: bool,
    pub(super) show_help: bool,
    pub(super) scroll_inverted: bool,
}

impl ViewportState {
    pub(super) fn new() -> Self {
        Self {
            scroll_top: 0,
            auto_follow_bottom: true,
            selection: None,
            mouse_drag_mode: MouseDragMode::Undecided,
            mouse_drag_last_row: 0,
            mobile_mouse_buffer: String::new(),
            mobile_mouse_last_y: None,
            mobile_plain_pending_coords: false,
            mobile_plain_suppress_coords: false,
            mobile_plain_last_direction: 0,
            mobile_plain_new_gesture: false,
            show_help: false,
            scroll_inverted: false,
        }
    }

    pub(super) fn sync_auto_follow_bottom(&mut self, max_scroll: usize) {
        if self.scroll_top >= max_scroll {
            self.scroll_top = max_scroll;
            self.auto_follow_bottom = true;
        } else {
            self.auto_follow_bottom = false;
        }
    }
}
