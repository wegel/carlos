//! Compose pending task arrows for every fresh row count allowed by the existing native ceiling.

use carlos_proto::tasks::MAX_TASK_ROWS;

/// Keep exact per-key saturation without retaining an unbounded list of keyboard actions.
pub(super) struct Move {
    shift: isize,
    bounds: Vec<[u16; 2]>,
}

impl Move {
    pub(super) fn new() -> Self {
        Self {
            shift: 0,
            bounds: (0..MAX_TASK_ROWS).map(|last| [0, last as u16]).collect(),
        }
    }

    /// Compose clamp(position + direction, 0, count - 1) for each possible fresh count.
    pub(super) fn push(&mut self, direction: isize) {
        let mut constant = true;
        for (last, bounds) in self.bounds.iter_mut().enumerate() {
            for bound in bounds.iter_mut() {
                *bound = (*bound as isize)
                    .saturating_add(direction)
                    .clamp(0, last as isize) as u16;
            }
            constant &= bounds[0] == bounds[1];
        }
        self.shift = if constant {
            0
        } else {
            self.shift.saturating_add(direction)
        };
    }

    /// Apply the composed keys only after fresh pages locate the original task identity.
    pub(super) fn apply(&self, position: usize, count: usize) -> usize {
        let [low, high] = self.bounds[count - 1];
        position
            .saturating_add_signed(self.shift)
            .clamp(low as usize, high as usize)
    }
}

#[cfg(test)]
#[path = "task_move_tests.rs"]
mod tests;
