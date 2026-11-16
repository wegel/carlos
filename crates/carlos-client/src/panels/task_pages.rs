//! Keep one bounded task page and native-independent opaque cursors while preserving selected identity.

use carlos_proto::ids::{TaskId, TaskRevision};
use carlos_proto::tasks::{Task, TaskPage, MAX_PAGE_TASKS, MAX_TASK_ROWS};

use super::task_move::Move;
use crate::ClientError;

#[derive(Default)]
enum Target {
    #[default]
    Selected,
    Position(usize),
    Relative {
        movement: Move,
        destination: Option<usize>,
    },
}

/// Keep previews for one page and addresses for at most the native task lists under the existing 4,352-row ceiling.
#[derive(Default)]
pub struct TaskPageWindow {
    revision: Option<TaskRevision>,
    entries: Vec<Task>,
    position: Option<usize>,
    settled: bool,
    start: usize,
    next_start: usize,
    cursors: Vec<Option<TaskId>>,
    target: Target,
}

impl TaskPageWindow {
    pub(crate) fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self
                .entries
                .iter()
                .map(|task| 512 + task.label.capacity() + task.state.capacity())
                .sum::<usize>()
            + self.cursors.len() * (64 + std::mem::size_of::<Option<TaskId>>())
    }

    /// Borrow the revision that proves the retained rows.
    pub fn revision(&self) -> Option<&TaskRevision> {
        self.revision.as_ref()
    }

    /// Borrow the one retained page, rather than all rows walked to locate a target.
    pub fn entries(&self) -> &[Task] {
        &self.entries
    }

    /// Borrow the proven global position of the selected opaque identity.
    pub fn position(&self) -> Option<usize> {
        self.position
    }

    /// Report whether the requested identity or ordered arrow target has settled.
    pub fn is_settled(&self) -> bool {
        self.settled
    }

    /// Require a fresh revision while keeping the last readonly row available for display.
    pub fn invalidate(&mut self) {
        self.revision = None;
    }

    /// Retire stale previews without selecting a different native entry.
    pub fn reset(&mut self, revision: TaskRevision, selected: bool) {
        let target = match std::mem::take(&mut self.target) {
            Target::Relative { movement, .. } if selected => Target::Relative {
                movement,
                destination: None,
            },
            Target::Position(index) if !self.settled => Target::Position(index),
            _ if selected => Target::Selected,
            _ => Target::Position(0),
        };
        *self = Self {
            revision: Some(revision),
            cursors: vec![None],
            target,
            ..Self::default()
        };
    }

    /// Borrow only the selected entry currently proven by this page's revision.
    pub fn selected(&self, selected: Option<&TaskId>) -> Option<&Task> {
        self.settled.then_some(())?;
        self.entries
            .iter()
            .find(|entry| Some(&entry.id) == selected)
    }

    /// Return the next explicit page address without retaining its preview text.
    pub fn next(&self) -> Option<(usize, Option<TaskId>)> {
        if self.settled {
            return None;
        }
        Some((
            self.next_start,
            self.cursors.get(self.next_start / MAX_PAGE_TASKS)?.clone(),
        ))
    }

    /// Move only after an explicit arrow, reading intervening pages when their addresses are unknown.
    pub fn navigate(
        &mut self,
        direction: isize,
        count: usize,
        selected: &mut Option<TaskId>,
    ) -> Result<(), ClientError> {
        if count > MAX_TASK_ROWS {
            return Err(ClientError::InvalidPayload(
                "Task count exceeds the supported native view".into(),
            ));
        }
        if count == 0 {
            *selected = None;
            self.position = None;
            return Ok(());
        }
        if self.defer_relative(direction, selected.is_some()) {
            return Ok(());
        }
        let current = match self.target {
            Target::Position(index) if !self.settled => Some(index),
            _ => self.position,
        };
        let index = current.map_or(0, |index| {
            index.saturating_add_signed(direction).min(count - 1)
        });
        self.target = Target::Position(index);
        self.settled = false;
        if let Some(entry) = index
            .checked_sub(self.start)
            .and_then(|index| self.entries.get(index))
        {
            *selected = Some(entry.id.clone());
            self.position = Some(index);
            self.settled = true;
        } else {
            if selected.is_some() && self.position.is_some() {
                let mut movement = Move::new();
                movement.push(direction);
                self.target = Target::Relative {
                    movement,
                    destination: Some(index),
                };
            }
            self.next_start =
                (index / MAX_PAGE_TASKS).min(self.cursors.len().saturating_sub(1)) * MAX_PAGE_TASKS;
        }
        Ok(())
    }

    /// Accept a bounded page, stopping once the selected id or requested ordinal is present.
    pub fn accept(
        &mut self,
        page: TaskPage,
        start: usize,
        count: usize,
        selected: &mut Option<TaskId>,
    ) -> Result<(), ClientError> {
        let end = self.checked_end(&page, start, count)?;
        if page.has_more {
            let next = start / MAX_PAGE_TASKS + 1;
            if next > self.cursors.len() {
                return Err(ClientError::InvalidPayload(
                    "Task page skipped an unknown cursor".into(),
                ));
            }
            let cursor = page.tasks.last().expect("nonempty page").id.clone();
            if next == self.cursors.len() {
                self.cursors.push(Some(cursor));
            } else {
                self.cursors[next] = Some(cursor);
            }
        }
        self.start = start;
        self.entries = page.tasks;
        self.position = self
            .entries
            .iter()
            .position(|entry| Some(&entry.id) == selected.as_ref())
            .map(|index| start + index);
        self.settled = self.settle_target(page.has_more, count, selected);
        self.next_start = if let Some(index) = self
            .target
            .destination()
            .filter(|index| !self.settled && *index < start)
        {
            (index / MAX_PAGE_TASKS).min(self.cursors.len().saturating_sub(1)) * MAX_PAGE_TASKS
        } else {
            end
        };
        Ok(())
    }

    fn defer_relative(&mut self, direction: isize, selected: bool) -> bool {
        let relative = matches!(self.target, Target::Relative { .. });
        let absolute = matches!(self.target, Target::Position(_));
        if self.settled || absolute || !selected || !relative && self.position.is_some() {
            return false;
        }
        let mut movement = match std::mem::take(&mut self.target) {
            Target::Relative { movement, .. } => movement,
            _ => Move::new(),
        };
        movement.push(direction);
        self.target = Target::Relative {
            movement,
            destination: None,
        };
        self.position = None;
        self.next_start = 0;
        true
    }

    fn checked_end(
        &self,
        page: &TaskPage,
        start: usize,
        count: usize,
    ) -> Result<usize, ClientError> {
        if self.revision.as_ref() != Some(&page.revision) {
            return Err(ClientError::StaleTasks);
        }
        let end = start
            .checked_add(page.tasks.len())
            .ok_or_else(|| ClientError::InvalidPayload("Task page interval overflowed".into()))?;
        if count > MAX_TASK_ROWS
            || end > count
            || page.tasks.len() > MAX_PAGE_TASKS
            || page.has_more && (page.tasks.len() != MAX_PAGE_TASKS || end >= count)
            || !page.has_more && end != count
        {
            return Err(ClientError::InvalidPayload(
                "Task page does not match its declared count; refresh before using this task"
                    .into(),
            ));
        }
        Ok(end)
    }

    fn settle_target(&mut self, more: bool, count: usize, selected: &mut Option<TaskId>) -> bool {
        match std::mem::take(&mut self.target) {
            Target::Selected => self.position.is_some() || !more,
            Target::Position(index) => {
                let index = index.min(count.saturating_sub(1));
                self.target = Target::Position(index);
                self.select_position(index, count, selected)
            }
            Target::Relative {
                movement,
                destination,
            } => self.settle_relative(movement, destination, more, count, selected),
        }
    }

    fn settle_relative(
        &mut self,
        movement: Move,
        destination: Option<usize>,
        more: bool,
        count: usize,
        selected: &mut Option<TaskId>,
    ) -> bool {
        let index = destination.or_else(|| {
            self.position
                .map(|position| movement.apply(position, count))
        });
        let Some(index) = index else {
            if more {
                self.target = Target::Relative {
                    movement,
                    destination: None,
                };
            }
            return !more;
        };
        if self.select_position(index, count, selected) {
            self.target = Target::Position(index);
            true
        } else {
            self.target = Target::Relative {
                movement,
                destination: Some(index),
            };
            false
        }
    }

    fn select_position(
        &mut self,
        index: usize,
        count: usize,
        selected: &mut Option<TaskId>,
    ) -> bool {
        if count == 0 {
            *selected = None;
            self.position = None;
            return true;
        }
        if let Some(entry) = index
            .checked_sub(self.start)
            .and_then(|index| self.entries.get(index))
        {
            *selected = Some(entry.id.clone());
            self.position = Some(index);
            true
        } else {
            false
        }
    }
}

impl Target {
    fn destination(&self) -> Option<usize> {
        match self {
            Self::Position(index) => Some(*index),
            Self::Relative { destination, .. } => *destination,
            Self::Selected => None,
        }
    }
}

#[cfg(test)]
#[path = "task_pages_tests.rs"]
mod tests;
