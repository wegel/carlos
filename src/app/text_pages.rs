//! Share exact wrapped row ranges and original byte maps with device text panels.

#[path = "transcript_source_ranges.rs"]
pub(in crate::app) mod source_ranges;
#[path = "transcript_text_window.rs"]
pub(in crate::app) mod text_window;

pub(in crate::app) use self::source_ranges::SourceRanges;
pub(in crate::app) use self::text_window::visit_ranges;
