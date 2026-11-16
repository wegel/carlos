//! Preserve complete queue walks, explicit movement and stale-row safety without a terminal.

use carlos_proto::ids::{QueueEntryId, QueueRevision};
use carlos_proto::queue::{QueuePage, MAX_PAGE_ENTRIES};
use serde_json::json;

use super::QueuePageWindow;
use crate::ClientError;

#[test]
fn a_selected_row_beyond_two_pages_is_found_without_retaining_the_prefix() {
    let mut window = QueuePageWindow::default();
    let mut selected = Some(id(64).to_string());
    window.reset(revision('a'), true);
    for start in [0, 32, 64] {
        let (position, after) = window.next().expect("next checked page");
        assert_eq!(position, start);
        assert_eq!(after, start.checked_sub(1).map(id));
        window
            .accept(page(start, 65, 'a'), start, &mut selected)
            .expect("valid page");
        assert!(window.entries().len() <= MAX_PAGE_ENTRIES);
    }
    assert_eq!(window.position(), Some(64));
    assert_eq!(window.entries().len(), 1);
    assert_eq!(
        window.selected(selected.as_deref()).expect("proven row").id,
        id(64)
    );
    assert!(window.next().is_none());
}

#[test]
fn explicit_wrap_reads_unknown_pages_and_keeps_the_old_row_until_the_target_arrives() {
    let mut window = QueuePageWindow::default();
    let mut selected = None;
    window.reset(revision('a'), false);
    window
        .accept(page(0, 65, 'a'), 0, &mut selected)
        .expect("first page");
    assert_eq!(selected, Some(id(0).to_string()));
    window.navigate(-1, 65, &mut selected);
    assert_eq!(selected, Some(id(0).to_string()));
    assert!(!window.is_settled());
    for start in [32, 64] {
        assert_eq!(window.next().expect("next page").0, start);
        window
            .accept(page(start, 65, 'a'), start, &mut selected)
            .expect("cursor walk");
    }
    assert_eq!(selected, Some(id(64).to_string()));
    window.navigate(1, 65, &mut selected);
    assert_eq!(window.next().expect("known first bookmark"), (0, None));
    window
        .accept(page(0, 65, 'a'), 0, &mut selected)
        .expect("wrapped page");
    assert_eq!(selected, Some(id(0).to_string()));
}

#[test]
fn a_replacement_list_does_not_silently_select_another_entry() {
    let mut window = QueuePageWindow::default();
    let mut selected = Some(id(64).to_string());
    window.reset(revision('b'), true);
    window
        .accept(page(0, 10, 'b'), 0, &mut selected)
        .expect("replacement list");
    assert!(window.is_settled());
    assert!(window.selected(selected.as_deref()).is_none());
    assert_eq!(selected, Some(id(64).to_string()));
    window.navigate(1, 10, &mut selected);
    assert_eq!(selected, Some(id(0).to_string()));
}

#[test]
fn invalid_intervals_and_foreign_revisions_leave_the_existing_window_untouched() {
    let mut window = QueuePageWindow::default();
    let mut selected = None;
    window.reset(revision('a'), false);
    window
        .accept(page(0, 1, 'a'), 0, &mut selected)
        .expect("original row");
    assert!(matches!(
        window.accept(page(0, 1, 'b'), 0, &mut selected),
        Err(ClientError::StaleQueue)
    ));
    assert!(window
        .accept(page(0, 1, 'a'), usize::MAX, &mut selected)
        .is_err());
    let mut bad = page(0, 65, 'a');
    bad.has_more = false;
    assert!(window.accept(bad, 0, &mut selected).is_err());
    assert_eq!(window.entries().len(), 1);
    assert_eq!(window.revision(), Some(&revision('a')));
    assert_eq!(
        window
            .selected(selected.as_deref())
            .expect("original preserved")
            .preview,
        "row 0"
    );
}

fn id(index: usize) -> QueueEntryId {
    format!("{index:064x}").parse().expect("opaque row id")
}

fn revision(letter: char) -> QueueRevision {
    letter.to_string().repeat(64).parse().expect("revision")
}

fn page(start: usize, count: usize, rev: char) -> QueuePage {
    let end = (start + MAX_PAGE_ENTRIES).min(count);
    let entries: Vec<_> = (start..end)
        .map(|index| {
            json!({
                "id": id(index), "client_message":"human", "preview":format!("row {index}"),
                "preview_complete":true, "text_bytes":format!("row {index}").len().to_string()
            })
        })
        .collect();
    serde_json::from_value(json!({
        "session":"a".repeat(32), "seq":"10", "queue":{
            "revision":revision(rev), "known":true,"reading":false,"pending":false,
            "count":count,"read_error":null,"notice":""
        }, "entries":entries,"has_more":end<count
    }))
    .expect("typed queue page")
}
