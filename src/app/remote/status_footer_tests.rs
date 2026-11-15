//! Keep narrow footer labels and private mode choices on the real daemon client.

use std::time::{Duration, Instant};

use carlos_core::session::usage::unix_now;
use carlos_proto::queue::QueueFacts;
use carlos_proto::session::{Backend, Context, Quota, SessionInfo};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::remote::widget_fixture::{device_with_info, info};
use crate::app::remote::Device;
use crate::app::view_models::TerminalSize;

fn session(backend: Backend) -> SessionInfo {
    let mut facts = info("a".repeat(32).parse().expect("session address"), false);
    facts.backend = backend;
    facts.context = Some(Context {
        used: 90_000,
        max: 100_000,
    });
    facts.quota = Some(Quota {
        remaining_pct: 50.0,
        resets_at: Some(unix_now() + 3 * 86_400),
    });
    facts
}

fn status(device: &Device, width: usize) -> String {
    let mut buffer = Buffer::empty(Rect::new(0, 0, width as u16, 3));
    let facts = device
        .client
        .session(&device.session)
        .expect("accepted session")
        .info();
    device.draw_status(&mut buffer, TerminalSize { width, height: 32 }, 1, facts);
    (0..width)
        .map(|column| buffer[(column as u16, 1)].symbol())
        .collect()
}

fn toggle(device: &mut Device) {
    device
        .event(
            None,
            Event::Key(KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE)),
        )
        .expect("local mode key");
}

#[test]
fn default_is_quiet_queue_is_visible_and_explicit_steer_notice_expires_without_native_work() {
    for backend in [Backend::Codex, Backend::Claude] {
        let mut device = device_with_info(Vec::new(), session(backend));
        for label in ["Steer", "F2", "F3"] {
            assert!(!status(&device, 100).contains(label));
        }
        assert!(!device.expire_queue_mode_notice());
        toggle(&mut device);
        assert!(status(&device, 100).contains("Queue"));
        toggle(&mut device);
        assert!(status(&device, 100).contains("Steer"));
        let now = Instant::now();
        let until = device
            .view
            .viewport
            .steer_notice_until
            .expect("two second notice");
        assert!(until > now && until <= now + Duration::from_secs(2));
        device.view.viewport.steer_notice_until = Some(now - Duration::from_millis(1));
        assert!(device.expire_queue_mode_notice());
        assert!(!status(&device, 100).contains("Steer"));
        assert!(!device.expire_queue_mode_notice());
        assert!(device.client.operations().iter().next().is_none());
        assert!(device.view.input.is_empty());
    }
}

#[test]
fn narrow_queue_reserves_compact_context_and_keeps_weekly_at_the_right_edge() {
    for backend in [Backend::Codex, Backend::Claude] {
        let mut device = device_with_info(Vec::new(), session(backend));
        toggle(&mut device);
        for width in [28, 36, 54] {
            let line = status(&device, width);
            assert!(line.contains("Queue"), "{width}: {line}");
            assert!(line.contains("90%"), "{width}: {line}");
            assert!(line.trim_end().ends_with("W50%/3.0d"), "{width}: {line}");
        }
        assert!(device.client.operations().iter().next().is_none());
    }
}

#[test]
fn narrow_default_mode_keeps_recovery_error_and_pending_notices_beside_quota_and_percent() {
    for (kind, full, compact) in [
        ("draft", "Unsent draft", "Draft"),
        ("error", "Queue error", "Q!"),
        ("pending", "Queue pending", "Q…"),
    ] {
        let facts = session(Backend::Codex);
        // Pending queue state is shared; failed or recovered drafts stay on this device.
        let mut device = device_with_info(Vec::new(), facts.clone());
        if kind == "pending" {
            let snapshot = carlos_proto::session::Snapshot {
                info: facts,
                entries: Vec::new(),
                approvals: Vec::new(),
                seq: carlos_proto::ids::Sequence::new(1),
                has_older: false,
                goal: None,
                settings: None,
                tasks: None,
                queue: Some(QueueFacts {
                    revision: "b".repeat(64).parse().expect("queue revision"),
                    known: true,
                    reading: false,
                    pending: true,
                    count: 0,
                    read_error: None,
                    notice: String::new(),
                }),
            };
            device = crate::app::remote::widget_fixture::device_with_snapshot(snapshot).0;
        }
        device.view.codex_queue.recovery = (kind == "draft").then(|| "retained draft".into());
        device.view.codex_queue.error = (kind == "error").then(|| "native rejected it".into());
        for width in [28, 36] {
            let line = status(&device, width);
            assert!(
                line.contains(if width < 32 { compact } else { full }),
                "{width}: {line}"
            );
            assert!(line.contains("90%"), "{width}: {line}");
            assert!(line.trim_end().ends_with("W50%/3.0d"), "{width}: {line}");
            assert!(!line.contains("Steer"));
        }
        assert!(device.client.operations().iter().next().is_none());
    }
}
