//! Count only one thread while callback workers prepare and send exact native replies.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::Decision;
use serde_json::{json, Value};

use super::approval_control_test_support::Peer;
use super::approval_state::{ApprovalChoice, ApprovalQueue};
use super::entry_id::EntryId;
use super::state::SessionState;
use crate::backend::BackendKind;
use crate::session::actions::ServerRequestAction;

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

fn request(kind: BackendKind, bytes: usize, form: bool) -> (SessionState, EntryId) {
    let mut core = SessionState::new("thread".into(), Default::default(), false);
    if kind == BackendKind::Codex {
        core.enable_codex();
        let (method, params) = if form {
            (
                "mcpServer/elicitation/request",
                json!({"threadId":"thread","serverName":"server",
                "mode":"form","message":"Answer","requestedSchema":{"type":"object","properties":{}},
                "padding":"p".repeat(bytes)}),
            )
        } else {
            (
                "item/commandExecution/requestApproval",
                json!({"threadId":"thread","turnId":"turn",
                "itemId":"item","command":"echo ok","padding":"p".repeat(bytes)}),
            )
        };
        core.accept_codex_callback(method, &json!({"id":"large","params":params}));
    } else {
        core.receive_claude_callback(&json!({"callbackId":"large","kind":"permission",
            "payload":{"toolName":"Bash","input":{"command":"echo ok"},"padding":"p".repeat(bytes)}}));
    }
    let id = core
        .approval()
        .pending()
        .expect("native request")
        .entry_id();
    (core, id)
}

fn command_reply_allocations(
    kind: BackendKind,
    decision: Decision,
    bytes: usize,
) -> (usize, usize) {
    let peer = Peer::new(kind);
    let (mut core, id) = request(kind, bytes, false);
    let count = allocations(|| {
        let work = core
            .prepare_engine_answer(&peer, id, ApprovalAnswer::Decision { decision })
            .expect("claim exact native card");
        core.finish_engine_control(&peer, work.execute(&peer.commands))
            .expect("settle exact answer");
    });
    assert_eq!(peer.calls.lock().expect("native answers").len(), 1);
    count
}

#[test]
fn command_replies_share_native_facts_without_copying_padding() {
    for kind in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        for decision in [Decision::Once, Decision::Deny, Decision::Cancel] {
            let small = command_reply_allocations(kind, decision, 0);
            let large = command_reply_allocations(kind, decision, 100_000);
            assert_eq!(large, small, "{kind:?} {decision:?}");
        }
    }
}

fn submitted_form(core: &mut SessionState, peer: &Peer, id: EntryId) {
    core.checked_codex_callback(&json!({"requestId":"large","revision":0,"ok":true}));
    core.answer_codex_callback(
        peer,
        id,
        &json!("large"),
        json!({"action":"accept","content":{}}),
        ApprovalChoice::Accept,
    )
    .expect("submit original candidate");
    assert!(peer.calls.lock().expect("native answers").is_empty());
    assert!(core.submitted_approval_answer(id).is_some());
}

fn checked_form(core: &mut SessionState) -> (EntryId, Value) {
    let update = core.checked_codex_callback(&json!({"requestId":"large","revision":1,"ok":true}));
    match update.action.expect("original submitted form may finish") {
        ServerRequestAction::ReplyNative { card_id, result } => (card_id, result),
        _ => panic!("only original native answer"),
    }
}

fn form_completion_allocations(bytes: usize) -> (usize, usize) {
    let peer = Peer::new(BackendKind::Codex);
    let (mut core, id) = request(BackendKind::Codex, bytes, true);
    submitted_form(&mut core, &peer, id);
    let (card, body) = checked_form(&mut core);
    let count = allocations(|| {
        let work = core
            .prepare_checked_native_answer(BackendKind::Codex, card, body)
            .expect("checked form");
        core.finish_engine_control(&peer, work.execute(&peer.commands))
            .expect("native form completed");
    });
    assert_eq!(
        peer.calls.lock().expect("native answers").as_slice(),
        &[(json!("large"), json!({"action":"accept","content":{}}))]
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

#[test]
fn an_answer_submitted_while_active_can_finish_validation_after_focus_moves() {
    let peer = Peer::new(BackendKind::Codex);
    let (mut core, submitted) = request(BackendKind::Codex, 0, true);
    submitted_form(&mut core, &peer, submitted);
    core.accept_codex_callback(
        "item/commandExecution/requestApproval",
        &json!({"id":"second",
        "params":{"threadId":"thread","turnId":"turn","itemId":"second","command":"pwd"}}),
    );
    let second = core.approval().queued(ApprovalQueue::Codex)[0].entry_id();
    core.focus_approval(second, ApprovalQueue::Codex);
    assert_ne!(
        core.approval().pending().expect("second").entry_id(),
        submitted
    );
    assert!(
        !core.begin_approval_answer(submitted),
        "queued cards cannot start old active-only submissions"
    );
    let (card, body) = checked_form(&mut core);
    assert_eq!(card, submitted);
    let work = core
        .prepare_checked_native_answer(BackendKind::Codex, card, body)
        .expect("same live claim");
    core.finish_engine_control(&peer, work.execute(&peer.commands))
        .expect("native completion");
    assert_eq!(
        peer.calls.lock().expect("native answers").as_slice(),
        &[(json!("large"), json!({"action":"accept","content":{}}))]
    );
    assert!(core.submitted_approval_answer(submitted).is_none());
    assert!(!core.begin_approval_answer(submitted));
    assert!(core
        .prepare_checked_native_answer(BackendKind::Codex, submitted, json!({"behavior":"allow"}))
        .is_none());
    assert_eq!(
        peer.calls.lock().expect("native answers").len(),
        1,
        "retired submissions cannot send again"
    );
}
