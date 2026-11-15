//! Paint the final native reasoning summary as separate Markdown paragraphs.

use carlos_core::session::models::Role;

use super::render_fixture;

#[test]
fn completed_reasoning_summary_keeps_markdown_rows() {
    let mut view = render_fixture::view(vec![render_fixture::row(Role::Reasoning,
        "**Designing fullscreen focus overrides**\n\nI’m working out a helper function.\n**Analyzing input handling conditions**")]);
    let rendered = render_fixture::snapshot(&mut view, 120);
    assert_eq!(rendered[0].text, "Designing fullscreen focus overrides");
    assert_eq!(rendered[1].text, "I’m working out a helper function.");
    assert_eq!(rendered[2].text, "Analyzing input handling conditions");
}
