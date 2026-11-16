//! Report native writer stages without retaining request bodies or extending a deadline.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

#[derive(Clone, Copy)]
/// Name the last completed step without retaining the frame that was written.
pub(crate) enum Stage {
    Dequeued,
    Checked,
    Ready,
    Written,
}

/// Share monotonic elapsed times between a native writer and its bounded waiter.
pub(crate) struct WriteTiming {
    enqueued: Instant,
    marks: [AtomicU64; 4],
}

impl WriteTiming {
    /// Start timing immediately before the frame enters the bounded queue.
    pub(crate) fn new() -> Self {
        Self {
            enqueued: Instant::now(),
            marks: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }

    /// Record a completed stage; Ready follows optional tracing and Written follows flush.
    pub(crate) fn mark(&self, stage: Stage) {
        let micros = self
            .enqueued
            .elapsed()
            .as_micros()
            .min(u64::MAX as u128 - 1) as u64;
        self.marks[stage as usize].store(micros + 1, Ordering::Release);
    }
}

impl fmt::Display for WriteTiming {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Read later stages first so an observed completion also exposes its earlier marks.
        let mut marks = [0; 4];
        for index in (0..4).rev() {
            marks[index] = self.marks[index].load(Ordering::Acquire);
        }
        write!(formatter, "enqueue+0us")?;
        for (name, value) in ["dequeued", "checked", "ready", "written"]
            .into_iter()
            .zip(marks)
        {
            if value == 0 {
                write!(formatter, "; {name}=pending")?;
            } else {
                write!(formatter, "; {name}+{}us", value - 1)?;
            }
        }
        write!(
            formatter,
            "; observed+{}us",
            self.enqueued.elapsed().as_micros()
        )
    }
}
