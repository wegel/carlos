//! Check global ordering, backward pages and fixed memory bounds across unordered native pages.

use super::window::Window;
use super::{PageAnchor, PageDirection, PAGE_ROWS};
use crate::session::models::ThreadSummary;

fn row(index: usize) -> ThreadSummary {
    ThreadSummary {
        id: format!("native-{index:05}"),
        name: Some(format!("Session {index}")),
        preview: String::new(),
        cwd: "/project".into(),
        created_at: index as i64,
        updated_at: index as i64,
    }
}

fn page(direction: PageDirection) -> super::SavedPage {
    let mut window = Window::new(direction);
    for index in (0..10000).step_by(2).chain((1..10000).step_by(2)) {
        window.push(row(index));
    }
    window.finish()
}

#[test]
fn large_unordered_catalogue_keeps_nearest_forward_and_backward_pages() {
    let newest = page(PageDirection::Newest);
    assert_eq!(newest.rows.len(), PAGE_ROWS);
    assert_eq!(newest.rows[0].id, "native-09999");
    assert_eq!(newest.rows[31].id, "native-09968");
    assert!(!newest.has_newer && newest.has_older);
    let older = page(PageDirection::Older(PageAnchor::from_row(&newest.rows[31])));
    assert_eq!(older.rows[0].id, "native-09967");
    assert_eq!(older.rows[31].id, "native-09936");
    assert!(older.has_newer && older.has_older);
    let back = page(PageDirection::Newer(PageAnchor::from_row(&older.rows[0])));
    assert_eq!(
        back.rows.iter().map(|row| &row.id).collect::<Vec<_>>(),
        newest.rows.iter().map(|row| &row.id).collect::<Vec<_>>()
    );
    assert!(!back.has_newer && back.has_older);
}

#[test]
fn ties_keep_native_ordering_keys_private_and_deduplicate_chosen_rows() {
    let mut window = Window::new(PageDirection::Newest);
    for index in (0..100).rev() {
        let mut row = row(index);
        row.created_at = 1;
        row.updated_at = 2;
        window.push(row.clone());
        window.push(row);
    }
    let page = window.finish();
    assert_eq!(page.rows.len(), PAGE_ROWS);
    assert_eq!(page.rows[0].id, "native-00000");
    assert_eq!(page.rows[31].id, "native-00031");
}

#[test]
fn previews_stop_at_utf8_boundaries_without_rewriting_native_addresses() {
    let mut window = Window::new(PageDirection::Newest);
    let mut valid = row(1);
    valid.name = Some("🦊".repeat(512));
    valid.preview = "é🦊".repeat(4096);
    window.push(valid);
    let mut invalid = row(2);
    invalid.id = "x".repeat(4097);
    window.push(invalid);
    let page = window.finish();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].id, "native-00001");
    assert_eq!(
        page.rows[0].name.as_deref(),
        Some("🦊".repeat(128).as_str())
    );
    assert!(page.rows[0].preview.len() <= 4096);
    assert!(page.rows[0].preview.starts_with("é🦊"));
}
