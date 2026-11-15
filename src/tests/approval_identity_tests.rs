//! Keep pending-card ids stable through duplicate delivery, focus, resolution and Claude clear.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use carlos_core::backend::BackendKind;
use carlos_core::session::approval_state::ApprovalChoice;
use carlos_core::session::entry_id::EntryId;
use serde_json::json;

use crate::app::approval_lifetime_support::{app, callback, server, Client};
use crate::app::codex_callbacks::{next, receive};
use crate::app::state::AppState;

fn active(app: &AppState) -> EntryId {
    app.session
        .approval()
        .pending()
        .expect("pending")
        .entry_id()
}

#[test]
fn an_answer_submitted_while_active_can_finish_validation_after_focus_moves() {
    use crate::app::approval_lifetime_support::{app, request, Client, METHODS};

    use crate::app::codex_callbacks::checked;
    use crate::app::input::input_dispatch::dispatch_action;
    use carlos_core::backend::BackendKind;
    use carlos_core::session::approval_state::ApprovalChoice;
    let client = Client::new(BackendKind::Codex);
    let mut app = app(BackendKind::Codex);
    receive(
        &mut app,
        "mcpServer/elicitation/request",
        &json!({"id":"form","params":{
        "threadId":"thread","serverName":"server","mode":"form","message":"Answer",
        "requestedSchema":{"type":"object","properties":{}}}}),
    );
    checked(
        &mut app,
        &json!({"requestId":"form","revision":0,"ok":true}),
    );
    let submitted = active(&app);
    crate::app::codex_reply::respond(&client, &mut app, ApprovalChoice::Accept);
    assert!(client.answers().is_empty());
    assert!(app.session.submitted_approval_answer(submitted).is_some());
    request(&client, &mut app, METHODS[0], "second");
    next(&mut app);
    assert_ne!(active(&app), submitted);
    assert!(
        !app.session.begin_approval_answer(submitted),
        "queued cards cannot start answers"
    );
    let action = checked(
        &mut app,
        &json!({"requestId":"form","revision":1,"ok":true}),
    )
    .expect("submitted form may finish while queued");
    dispatch_action(&client, &mut app, action);
    assert_eq!(
        client.answers(),
        [json!(["answer","form",{"action":"accept","content":{}}])]
    );
    assert!(app.session.submitted_approval_answer(submitted).is_none());
    assert!(!app.session.begin_approval_answer(submitted));
    crate::app::codex_reply::send(&client, &mut app, submitted, json!({"behavior":"allow"}));
    assert_eq!(
        client.answers().len(),
        1,
        "retired submissions cannot send again"
    );
}

// Count only this thread's measured reply, excluding setup and concurrent tests.
thread_local! {
    static ALLOCATIONS: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

struct CountingAllocator;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn count_allocation(bytes: usize) {
    let _ = ALLOCATIONS.try_with(|count| {
        if let Some((calls, total)) = count.get() {
            count.set(Some((calls + 1, total + bytes)));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation(layout.size());
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count_allocation(layout.size());
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count_allocation(size);
        System.realloc(ptr, layout, size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }
}

struct AllocationScope;

impl Drop for AllocationScope {
    fn drop(&mut self) {
        ALLOCATIONS.with(|count| count.set(None));
    }
}

fn allocations(reply: impl FnOnce()) -> (usize, usize) {
    let _scope = AllocationScope;
    ALLOCATIONS.with(|count| count.set(Some((0, 0))));
    reply();
    ALLOCATIONS.with(|count| count.get().expect("counting reply"))
}

fn command_reply_allocations(
    backend: BackendKind,
    choice: ApprovalChoice,
    bytes: usize,
) -> (usize, usize) {
    let client = Client::new(backend);
    let mut app = app(backend);
    if backend == BackendKind::Codex {
        server(
            &client,
            &mut app,
            json!({"id":"large",
            "method":"item/commandExecution/requestApproval","params":{
            "threadId":"thread","turnId":"turn","itemId":"item",
            "command":"echo ok","padding":"p".repeat(bytes)}}),
        );
    } else {
        callback(
            &client,
            &mut app,
            "large",
            "permission",
            json!({"toolName":"Bash","input":{"command":"echo ok"},
                "padding":"p".repeat(bytes)}),
        );
    }
    let count = allocations(|| {
        crate::app::turn_approval::respond_to_pending_approval(&client, &mut app, choice);
    });
    assert_eq!(client.answers().len(), 1);
    count
}

#[test]
fn command_replies_copy_large_request_facts_only_once_for_each_backend() {
    for backend in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        for choice in [
            ApprovalChoice::Accept,
            ApprovalChoice::Decline,
            ApprovalChoice::Cancel,
        ] {
            let small = command_reply_allocations(backend, choice, 0);
            let large = command_reply_allocations(backend, choice, 100_000);
            assert_eq!(
                large,
                (small.0 + 1, small.1 + 100_000),
                "{backend:?} {choice:?}"
            );
        }
    }
}

fn form_completion_allocations(bytes: usize) -> (usize, usize) {
    let client = Client::new(BackendKind::Codex);
    let mut app = app(BackendKind::Codex);
    receive(
        &mut app,
        "mcpServer/elicitation/request",
        &json!({"id":"large",
        "params":{"threadId":"thread","serverName":"server","mode":"form",
        "message":"Answer","requestedSchema":{"type":"object","properties":{}},
        "padding":"p".repeat(bytes)}}),
    );
    crate::app::codex_callbacks::checked(
        &mut app,
        &json!({"requestId":"large","revision":0,"ok":true}),
    );
    crate::app::codex_reply::respond(&client, &mut app, ApprovalChoice::Accept);
    assert!(client.answers().is_empty());
    let action = crate::app::codex_callbacks::checked(
        &mut app,
        &json!({"requestId":"large","revision":1,"ok":true}),
    )
    .expect("validated answer");
    let count = allocations(|| {
        crate::app::input::input_dispatch::dispatch_action(&client, &mut app, action);
    });
    assert_eq!(
        client.answers(),
        [json!(["answer","large",{"action":"accept","content":{}}])]
    );
    count
}

#[test]
fn validated_form_completion_does_not_copy_request_facts() {
    assert_eq!(
        form_completion_allocations(0),
        form_completion_allocations(100_000)
    );
}
