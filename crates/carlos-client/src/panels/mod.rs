//! Keep device-owned panel read windows separate from accepted session facts and UI editors.

mod queue_pages;

pub use queue_pages::QueuePageWindow;

#[cfg(test)]
mod queue_pages_tests;
