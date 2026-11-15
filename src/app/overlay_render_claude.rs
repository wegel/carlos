//! Adapt embedded callback facts to the shared borrowed question-card painter.

use carlos_core::session::approval_state::{
    ApprovalRequestDraft, ApprovalRequestKind, PendingApprovalRequest,
};
use ratatui::buffer::Buffer;

use super::question_card::{self, QuestionCard};
use crate::app::claude_question_state::NativeDraft;
use crate::app::claude_question_view::{body_lines, caller, QuestionControls};
use crate::app::state::AppState;
use crate::app::TerminalSize;

pub(super) fn draw(
    buf: &mut Buffer,
    size: TerminalSize,
    app: &AppState,
    pending: &PendingApprovalRequest,
    draft: &NativeDraft,
) {
    let display = display_facts(app, pending);
    let caller = caller(&display);
    let body = body_lines(app, pending, draft);
    question_card::draw(
        buf,
        size,
        QuestionCard {
            title: &display.title,
            caller: &caller,
            body: &body,
            count: crate::app::native_questions::count(app),
            controls: controls(app, pending),
        },
        draft,
    );
}

pub(super) fn cursor(
    size: TerminalSize,
    draft: &NativeDraft,
    pending: &PendingApprovalRequest,
    app: &AppState,
) -> Option<(usize, usize)> {
    question_card::cursor(size, draft, controls(app, pending))
}

fn controls(app: &AppState, pending: &PendingApprovalRequest) -> QuestionControls {
    let blocking = (pending.facts().kind == ApprovalRequestKind::CodexQuestion)
        .then(|| {
            app.session
                .codex_callbacks()
                .get(&pending.facts().request_id)
                .map(|entry| entry.blocking())
        })
        .flatten();
    QuestionControls::from_native(pending.facts(), blocking)
}

fn display_facts(app: &AppState, pending: &PendingApprovalRequest) -> ApprovalRequestDraft {
    let mut display = pending.to_draft();
    if let Some(entry) = app
        .session
        .codex_callbacks()
        .get(&pending.facts().request_id)
    {
        let mut payload = entry.params().clone();
        if payload["threadId"].is_null() {
            payload["threadId"] = payload["conversationId"].clone();
        }
        display.requested_permissions = Some(payload);
    }
    display
}
