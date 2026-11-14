//! Keep compact task publication independent of streamed root text and private accepted inputs.

use carlos_proto::bytes::ByteOffset;
use carlos_proto::session::Backend;
use carlos_proto::task_overview::{TaskOverviewData, TaskOverviewField};
use serde_json::{json, to_string};

use crate::session::events::Change;
use crate::session::models::Role;
use crate::session::runtime_settings::RuntimeDefaults;
use crate::session::state::SessionState;
use crate::session::usage::unix_now;

fn session(description: &str) -> SessionState {
    let mut session = SessionState::new("native-root".into(), RuntimeDefaults::default(), false);
    session.set_client_identity(Backend::Claude, "host".into());
    session.claude.enabled = true;
    session
        .update_claude_task(&json!({"subtype":"task_started","task_id":"native-task",
        "run_id":"native-run-1","description":description}))
        .expect("native task");
    session.publish();
    session
}

#[test]
fn root_text_deltas_never_rehash_or_copy_retained_native_task_bodies() {
    let mut core = session(&"description 🌍".repeat(80000));
    let revision = core.export_tasks().expect("task facts").revision;
    let before = (
        core.task_publication.metadata_rebuilds,
        core.task_publication.overview_rebuilds,
    );
    for index in 0..64 {
        core.append_message(Role::Assistant, format!("streamed root text {index}"));
        let batch = core.publish();
        assert!(!batch
            .records()
            .iter()
            .any(|record| record.change == Change::Tasks));
    }
    assert_eq!(
        core.export_tasks().expect("unchanged facts").revision,
        revision
    );
    assert_eq!(
        (
            core.task_publication.metadata_rebuilds,
            core.task_publication.overview_rebuilds
        ),
        before
    );
}

#[test]
fn overview_changes_preserve_task_metadata_revision_and_never_export_accepted_prompt_text() {
    let mut core = session("small task");
    let original = core.export_tasks().expect("initial facts");
    let rebuilds = core.task_publication.metadata_rebuilds;
    core.record_claude_thinking(&json!({"estimated_tokens":43}));
    core.register_claude_prompt(
        &json!({"uuid":"native-prompt","text":"PRIVATE_ACCEPTED_SOURCE",
        "origin":"human","priority":"later"}),
    )
    .expect("accepted input");
    core.claude_quota_event(
        &json!({"rateLimitType":"five_hour","utilization":0.25,"resetsAt":unix_now()+3600}),
    );
    assert!(core
        .publish()
        .records()
        .iter()
        .any(|record| record.change == Change::Tasks));
    let updated = core.export_tasks().expect("overview facts");
    assert_eq!(updated.revision, original.revision);
    assert_ne!(updated.overview_revision, original.overview_revision);
    assert_eq!(core.task_publication.metadata_rebuilds, rebuilds);
    let page = core
        .task_overview_page(Some(&updated.overview_revision), None, 32)
        .expect("typed overview");
    assert!(page.entries.iter().any(|entry| matches!(
        &entry.data,
        TaskOverviewData::Main {
            thinking_tokens: Some(43),
            ..
        }
    )));
    assert!(page.entries.iter().any(
        |entry| matches!(&entry.data,TaskOverviewData::Quota{used_pct,..} if *used_pct == 25.0)
    ));
    assert!(page.entries.iter().any(|entry| matches!(
        &entry.data,
        TaskOverviewData::Prompt {
            consumed: false,
            steer: false,
            ..
        }
    )));
    assert!(!to_string(&page)
        .expect("overview JSON")
        .contains("PRIVATE_ACCEPTED_SOURCE"));
}

#[test]
fn task_run_changes_publish_new_metadata_without_leaking_child_text_into_the_root() {
    let mut core = session("first child description");
    let original = core.export_tasks().expect("initial task");
    core.update_claude_task(&json!({"subtype":"task_started","task_id":"native-task",
        "run_id":"native-run-2","description":"PRIVATE_CHILD_DETAIL"}))
        .expect("replacement run");
    let snapshot = core.publish().snapshot(100);
    assert_ne!(
        snapshot.tasks.expect("new task facts").revision,
        original.revision
    );
    assert!(snapshot.entries.is_empty());
}

#[test]
fn overview_names_read_full_unicode_with_checked_revisions_and_exact_eof() {
    let mut core = session("task");
    let name = "🌍".repeat(700);
    core.claude_quota_optional(
        &json!({"windows":[{"key":"bucket","label":name,
        "usedPercent":12.5,"resetsAt":unix_now()+3600}]}),
        0,
    );
    core.publish();
    let facts = core.export_tasks().expect("quota facts");
    let page = core
        .task_overview_page(Some(&facts.overview_revision), None, 32)
        .expect("quota page");
    let quota = page
        .entries
        .iter()
        .find(|entry| matches!(&entry.data, TaskOverviewData::Quota { .. }))
        .expect("quota row");
    assert!(
        matches!(&quota.data,TaskOverviewData::Quota{label,..} if !label.complete && label.bytes.get() == name.len() as u64)
    );
    let text = core
        .task_overview_text(
            &facts.overview_revision,
            &quota.id,
            TaskOverviewField::Label,
            ByteOffset::new(2049),
            4,
        )
        .expect("Unicode floor");
    assert_eq!(
        (
            text.offset.get(),
            text.next_offset.get(),
            text.text.as_str()
        ),
        (2048, 2052, "🌍")
    );
    let eof = core
        .task_overview_text(
            &facts.overview_revision,
            &quota.id,
            TaskOverviewField::Label,
            ByteOffset::new(name.len() as u64),
            4,
        )
        .expect("exact EOF");
    assert!(eof.complete && eof.text.is_empty());
    core.record_claude_thinking(&json!({"estimated_tokens":99}));
    core.publish();
    assert!(core
        .task_overview_text(
            &facts.overview_revision,
            &quota.id,
            TaskOverviewField::Label,
            ByteOffset::new(0),
            4
        )
        .is_err());
}

#[test]
fn child_callback_counts_publish_without_rehashing_native_metadata_or_overview() {
    let mut core = SessionState::new("native-root".into(), Default::default(), false);
    core.enable_codex();
    core.set_codex_agents_for_test(vec![json!({"id":"child", "status":{"type":"idle"},
        "lastObservedTurn":{"id":"run","status":"completed"},"description":"large".repeat(200000)})]);
    core.verify_codex_agent_owners_for_test(core.codex_scope().epoch());
    core.publish();
    let original = core.export_tasks().expect("initial task counts");
    let rebuilds = (
        core.task_publication.metadata_rebuilds,
        core.task_publication.overview_rebuilds,
    );
    assert_eq!(original.counts.active, 0);
    let frame = json!({"id":"callback", "params":{"threadId":"child","turnId":"run", "itemId":"command", "command":"true"}});
    core.route_codex_callback(
        "item/commandExecution/requestApproval",
        &frame,
        core.backend_context().panel,
    )
    .expect("native child callback");
    assert!(core
        .publish()
        .records()
        .iter()
        .any(|record| record.change == Change::Tasks));
    let active = core.export_tasks().expect("waiting child counts");
    assert_eq!(active.counts.active, 1);
    assert_eq!(active.revision, original.revision);
    assert_eq!(active.overview_revision, original.overview_revision);
    core.retire_codex_callback(&json!("callback"));
    assert!(core
        .publish()
        .records()
        .iter()
        .any(|record| record.change == Change::Tasks));
    assert_eq!(
        core.export_tasks().expect("retired callback counts"),
        original
    );
    assert_eq!(
        (
            core.task_publication.metadata_rebuilds,
            core.task_publication.overview_rebuilds
        ),
        rebuilds
    );
}
