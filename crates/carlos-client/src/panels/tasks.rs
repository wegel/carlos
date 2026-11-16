//! Keep one task page and captured older-window rights beside the session's single task header.

use carlos_proto::commands::RequestId;
use carlos_proto::ids::{Sequence, TaskId, TaskRevision, TaskWindowId};
use carlos_proto::task_facts::TaskSnapshot;
use carlos_proto::task_windows::TaskWindow;
use carlos_proto::tasks::{Task, TaskPage};

use super::TaskPageWindow;

pub(crate) struct PageRead {
    pub id: RequestId,
    pub start: usize,
    pub sequence: Sequence,
}

pub(crate) struct Failure {
    pub sequence: Sequence,
    pub text: String,
}

#[derive(Default)]
pub(crate) struct TaskReads {
    pub header: Option<RequestId>,
    pub header_error: Option<String>,
    pub future_header: Option<TaskSnapshot>,
    pub page: Option<PageRead>,
    pub future_page: Option<(PageRead, TaskPage)>,
    pub failure: Option<Failure>,
}

#[derive(Default)]
pub(crate) struct WindowRead {
    pub request: Option<RequestId>,
    pub header: Option<TaskWindow>,
}

#[derive(Default)]
pub(crate) struct TaskWindows {
    pub current: Option<TaskWindow>,
    pub pending: Option<WindowRead>,
    pub stale: bool,
    pub close: Vec<TaskWindowId>,
    // Retain allocating reads until their exact window id returns, bounded by Requests.
    pub retired: Vec<RequestId>,
    pub error: Option<String>,
}

/// One device's checked task-list window, never a second authoritative task header or stop consent.
#[derive(Default)]
pub struct TaskPanel {
    pub(crate) pages: TaskPageWindow,
    pub(crate) selected: Option<TaskId>,
    pub(crate) reads: TaskReads,
    pub(crate) windows: TaskWindows,
    pub(crate) wanted: bool,
    pub(crate) paused: bool,
    pub(crate) refresh: bool,
    pub(crate) dirty: bool,
    pub(crate) count: usize,
    pub(crate) current: bool,
    pub(crate) charge: usize,
}

impl TaskPanel {
    /// Borrow the one retained page, including readonly rows while facts refresh.
    pub fn pages(&self) -> &TaskPageWindow {
        &self.pages
    }

    /// Borrow the original selected identity even when its row disappears or the socket closes.
    pub fn selected_id(&self) -> Option<&TaskId> {
        self.selected.as_ref()
    }

    /// Borrow a selected row only after its page and requested movement settle.
    pub fn selected(&self) -> Option<&Task> {
        self.pages.selected(self.selected.as_ref())
    }

    /// Borrow the accepted captured older header without exposing mutable native scope.
    pub fn window(&self) -> Option<&TaskWindow> {
        self.windows.current.as_ref()
    }

    /// Distinguish retained older rows from a live native window after close or EOF.
    pub fn window_is_stale(&self) -> bool {
        self.windows.stale
    }

    /// Report a candidate older window that has not replaced the accepted current page yet.
    pub fn is_reading_older(&self) -> bool {
        self.windows.pending.is_some()
    }

    /// Report the count checked against the existing native task-list allowance.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Borrow a header, page or older-window failure without replacing private stop errors.
    pub fn read_error(&self) -> Option<&str> {
        self.windows
            .error
            .as_deref()
            .or(self.reads.header_error.as_deref())
            .or_else(|| {
                self.reads
                    .failure
                    .as_ref()
                    .map(|failure| failure.text.as_str())
            })
    }

    /// Borrow the revision proving the displayed page rather than infer an older scope from latest facts.
    pub fn revision(&self) -> Option<&TaskRevision> {
        self.pages.revision()
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + 4096
            + self.charge
            + self.reads.header_error.as_ref().map_or(0, String::capacity)
            + self
                .reads
                .failure
                .as_ref()
                .map_or(0, |failure| failure.text.capacity())
            + self.windows.error.as_ref().map_or(0, String::capacity)
            + self.windows.retired.capacity() * std::mem::size_of::<RequestId>()
            + self.windows.close.capacity() * 128
            + self
                .windows
                .current
                .as_ref()
                .and_then(|header| header.read_error.as_ref())
                .map_or(0, String::capacity)
            + self
                .windows
                .pending
                .as_ref()
                .and_then(|pending| pending.header.as_ref())
                .and_then(|header| header.read_error.as_ref())
                .map_or(0, String::capacity)
            + self.reads.future_header.as_ref().map_or(0, |header| {
                512 + header.tasks.notice.capacity()
                    + header.tasks.read_error.as_ref().map_or(0, String::capacity)
            })
            + self.reads.future_page.as_ref().map_or(0, |(_, page)| {
                page.tasks
                    .iter()
                    .map(|task| 512 + task.label.capacity() + task.state.capacity())
                    .sum::<usize>()
            })
    }

    pub(crate) fn accept_window_header(&mut self, header: TaskWindow) {
        let pending = self.windows.pending.as_mut().expect("owned candidate");
        pending.request = None;
        if let Some(error) = header.read_error.as_ref().filter(|_| !header.known) {
            self.windows.error = Some(error.clone());
            self.windows.close.push(header.window);
            self.windows.pending = None;
        } else {
            pending.header = Some(header);
        }
    }

    pub(crate) fn charge_pages(&mut self) {
        self.charge = self
            .pages
            .retained_bytes()
            .saturating_sub(std::mem::size_of::<TaskPageWindow>());
    }

    pub(crate) fn disconnected(&mut self) {
        self.reads.header = None;
        self.reads.future_header = None;
        self.reads.page = None;
        self.reads.future_page = None;
        self.windows.pending = None;
        self.windows.close.clear();
        self.windows.retired.clear();
        self.windows.stale |= self.windows.current.is_some();
        self.paused = false;
        self.current = false;
        self.dirty = true;
    }
}
