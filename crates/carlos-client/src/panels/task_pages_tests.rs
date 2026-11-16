//! Preserve explicit relative navigation until a new revision resolves the original opaque id.

use carlos_proto::ids::{Sequence, TaskId};
use carlos_proto::tasks::{Task, TaskKind, TaskPage, MAX_TASK_ROWS};

use super::TaskPageWindow as Pages;
use crate::ClientError;

fn id(index: usize) -> TaskId {
    format!("{index:064x}").parse().expect("opaque task")
}
fn page(version: &str, ids: &[usize], more: bool) -> TaskPage {
    TaskPage {
        session: "a".repeat(32).parse().expect("session"),
        seq: Sequence::default(),
        revision: version.repeat(64).parse().expect("revision"),
        known: true,
        reading: false,
        closed: false,
        has_more: more,
        native_older_available: false,
        read_error: None,
        tasks: ids
            .iter()
            .map(|index| Task {
                id: id(*index),
                kind: TaskKind::Agent,
                label: format!("task{index}"),
                label_complete: true,
                state: "active".into(),
                run: None,
            })
            .collect(),
    }
}
fn reset(pages: &mut Pages, version: &str, selected: bool) {
    pages.reset(version.repeat(64).parse().expect("revision"), selected);
}

#[test]
fn arrows_during_a_revision_gap_follow_the_fresh_selected_identity() {
    let mut pages = Pages::default();
    let mut selected = None;
    reset(&mut pages, "a", false);
    pages
        .accept(page("a", &[0, 1, 2], false), 0, 3, &mut selected)
        .expect("initial page");
    reset(&mut pages, "b", true);
    pages
        .navigate(1, 3, &mut selected)
        .expect("valid task target");
    assert_eq!(selected, Some(id(0)));
    pages
        .accept(page("b", &[2, 0, 1], false), 0, 3, &mut selected)
        .expect("reordered page");
    assert_eq!(selected, Some(id(1)));
    assert_eq!(pages.position, Some(2));
}

#[test]
fn a_relative_move_can_seek_a_previously_read_page_after_resolving_identity() {
    let mut pages = Pages::default();
    let mut selected = None;
    reset(&mut pages, "a", false);
    let first: Vec<_> = (0..32).collect();
    let last: Vec<_> = (32..45).collect();
    pages
        .accept(page("a", &first, true), 0, 45, &mut selected)
        .expect("first page");
    pages
        .navigate(33, 45, &mut selected)
        .expect("valid task target");
    pages
        .accept(page("a", &last, false), 32, 45, &mut selected)
        .expect("selected page");
    assert_eq!(selected, Some(id(33)));
    reset(&mut pages, "b", true);
    pages
        .navigate(-2, 45, &mut selected)
        .expect("valid task target");
    pages
        .accept(page("b", &first, true), 0, 45, &mut selected)
        .expect("scan first page");
    pages
        .accept(page("b", &last, false), 32, 45, &mut selected)
        .expect("scan selected identity");
    assert_eq!(pages.next().expect("seek previous page").0, 0);
    pages
        .accept(page("b", &first, true), 0, 45, &mut selected)
        .expect("previous target page");
    assert_eq!(selected, Some(id(31)));
}

#[test]
fn pending_arrows_accumulate_without_selecting_a_removed_identity() {
    let mut pages = Pages::default();
    let mut selected = Some(id(1));
    reset(&mut pages, "b", true);
    pages
        .navigate(1, 4, &mut selected)
        .expect("valid task target");
    pages
        .navigate(1, 4, &mut selected)
        .expect("valid task target");
    reset(&mut pages, "c", true);
    pages
        .accept(page("c", &[0, 1, 2, 3], false), 0, 4, &mut selected)
        .expect("new facts");
    assert_eq!(selected, Some(id(3)));
    reset(&mut pages, "d", true);
    pages
        .navigate(-1, 3, &mut selected)
        .expect("valid task target");
    pages
        .accept(page("d", &[0, 1, 2], false), 0, 3, &mut selected)
        .expect("removed selected task");
    assert!(pages.settled);
    assert_eq!(selected, Some(id(3)));
    pages
        .navigate(1, 3, &mut selected)
        .expect("valid task target");
    assert_eq!(selected, Some(id(0)));
}

#[test]
fn another_revision_between_resolving_identity_and_destination_keeps_the_move() {
    let mut pages = Pages::default();
    let mut selected = Some(id(33));
    let first: Vec<_> = (0..32).collect();
    let last: Vec<_> = (32..45).collect();
    reset(&mut pages, "b", true);
    pages
        .navigate(-2, 45, &mut selected)
        .expect("valid task target");
    pages
        .accept(page("b", &first, true), 0, 45, &mut selected)
        .expect("scan first page");
    pages
        .accept(page("b", &last, false), 32, 45, &mut selected)
        .expect("locate original identity");
    assert_eq!(pages.next().expect("unread destination page").0, 0);
    reset(&mut pages, "c", true);
    pages
        .accept(page("c", &first, true), 0, 45, &mut selected)
        .expect("scan new first page");
    pages
        .accept(page("c", &last, false), 32, 45, &mut selected)
        .expect("locate original in new revision");
    pages
        .accept(page("c", &first, true), 0, 45, &mut selected)
        .expect("read fresh destination");
    assert_eq!(selected, Some(id(31)));
}

#[test]
fn an_arrow_started_on_a_ready_page_survives_a_revision_before_the_next_page() {
    let mut pages = Pages::default();
    let mut selected = Some(id(31));
    let first: Vec<_> = (0..32).collect();
    let last: Vec<_> = (32..45).collect();
    reset(&mut pages, "a", true);
    pages
        .accept(page("a", &first, true), 0, 45, &mut selected)
        .expect("selected first page");
    pages
        .navigate(1, 45, &mut selected)
        .expect("valid task target");
    pages
        .navigate(1, 45, &mut selected)
        .expect("valid task target");
    assert_eq!(selected, Some(id(31)));
    reset(&mut pages, "b", true);
    pages
        .accept(page("b", &first, true), 0, 45, &mut selected)
        .expect("fresh original identity");
    pages
        .accept(page("b", &last, false), 32, 45, &mut selected)
        .expect("fresh destination");
    assert_eq!(selected, Some(id(33)));
}

#[test]
fn arrows_before_the_first_page_remain_bounded_when_a_new_list_is_shorter() {
    let mut pages = Pages::default();
    let mut selected = None;
    reset(&mut pages, "a", false);
    pages
        .navigate(2, 3, &mut selected)
        .expect("valid task target");
    pages
        .navigate(2, 3, &mut selected)
        .expect("valid task target");
    reset(&mut pages, "b", false);
    pages
        .accept(page("b", &[0], false), 0, 1, &mut selected)
        .expect("shorter first list");
    assert!(pages.settled);
    assert_eq!(selected, Some(id(0)));
}

#[test]
fn choosing_another_row_after_removal_survives_a_revision_while_seeking_first_page() {
    let mut pages = Pages::default();
    let mut selected = Some(id(33));
    let first: Vec<_> = (0..32).collect();
    let last: Vec<_> = (32..45).filter(|index| *index != 33).collect();
    reset(&mut pages, "b", true);
    pages
        .accept(page("b", &first, true), 0, 44, &mut selected)
        .expect("scan first page");
    pages
        .accept(page("b", &last, false), 32, 44, &mut selected)
        .expect("selected identity absent");
    assert!(pages.settled);
    assert_eq!(pages.position, None);
    pages
        .navigate(1, 44, &mut selected)
        .expect("valid task target");
    pages
        .navigate(1, 44, &mut selected)
        .expect("valid task target");
    reset(&mut pages, "c", true);
    pages
        .accept(page("c", &first, true), 0, 44, &mut selected)
        .expect("explicit replacement target arrives");
    assert!(pages.settled);
    assert_eq!(selected, Some(id(1)));
}

#[test]
fn opposite_arrows_at_a_boundary_keep_their_exact_order_during_a_gap() {
    let mut pages = Pages::default();
    let mut selected = Some(id(0));
    reset(&mut pages, "b", true);
    pages
        .navigate(-1, 3, &mut selected)
        .expect("valid task target");
    pages
        .navigate(1, 3, &mut selected)
        .expect("valid task target");
    pages
        .accept(page("b", &[0, 1, 2], false), 0, 3, &mut selected)
        .expect("fresh first identity");
    assert_eq!(selected, Some(id(1)));
}

#[test]
fn unsupported_counts_and_extreme_moves_never_overflow_or_silently_truncate_a_list() {
    let mut pages = Pages::default();
    let mut selected = None;
    reset(&mut pages, "a", false);
    pages
        .accept(page("a", &[0, 1, 2], false), 0, 3, &mut selected)
        .expect("current rows");
    assert!(pages.navigate(1, MAX_TASK_ROWS + 1, &mut selected).is_err());
    assert_eq!(selected, Some(id(0)));
    pages
        .navigate(isize::MAX, 3, &mut selected)
        .expect("clamp to last row");
    assert_eq!(selected, Some(id(2)));
    pages
        .navigate(isize::MIN, 3, &mut selected)
        .expect("clamp to first row");
    assert_eq!(selected, Some(id(0)));
}

#[test]
fn a_bad_interval_or_revision_leaves_the_accepted_page_and_target_unchanged() {
    let mut pages = Pages::default();
    let mut selected = None;
    reset(&mut pages, "a", false);
    pages
        .accept(page("a", &[0], false), 0, 1, &mut selected)
        .expect("current row");
    assert!(matches!(
        pages.accept(page("b", &[1], false), 0, 1, &mut selected),
        Err(ClientError::StaleTasks)
    ));
    assert!(pages
        .accept(page("a", &[1], false), usize::MAX, 1, &mut selected)
        .is_err());
    assert!(pages
        .accept(page("a", &[1], true), 0, 1, &mut selected)
        .is_err());
    assert_eq!(selected, Some(id(0)));
    assert_eq!(
        pages
            .selected(selected.as_ref())
            .expect("original row")
            .label,
        "task0"
    );
}
