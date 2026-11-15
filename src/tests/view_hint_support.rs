//! Capture the remaining legacy lifetime oracle until its native successor is joined.

use carlos_core::session::approval_state::{ApprovalRequestDraft, ApprovalRequestKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use serde_json::json;

use super::state::AppState;
use super::view_models::TerminalSize;

pub(super) fn request(kind: ApprovalRequestKind, id: &str) -> ApprovalRequestDraft {
    carlos_core::session::approval_state::ApprovalRequestDraft {
        request_id: json!(id),
        method: "claude/native".into(),
        kind,
        title: format!("Approve {kind:?}"),
        detail_lines: vec!["native request details".into()],
        requested_permissions: Some(json!({"defaultToNo":true,"toolName":"Bash",
            "requestedSchema":{"type":"object","properties":{}}})),
        can_accept_for_session: true,
        can_decline: true,
        can_cancel: true,
    }
}

pub(super) fn card(app: &AppState) -> String {
    let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
    super::overlay_render::draw_approval_overlay(
        &mut buf,
        TerminalSize {
            width: 100,
            height: 30,
        },
        app,
    );
    let cells: Vec<_> = buf
        .content
        .iter()
        .map(|c| format!("{}:{:?}", c.symbol(), c.style()))
        .collect();
    blake3::hash(cells.join("\n").as_bytes())
        .to_hex()
        .to_string()
}
