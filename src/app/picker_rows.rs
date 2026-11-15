//! Borrow only the plain row fields used by the terminal picker painter.

/// Paint either a legacy row or a daemon's opaque saved-session choice without copying its strings.
#[derive(Clone, Copy)]
pub(in crate::app) struct PickerRow<'a> {
    pub id: &'a str,
    pub name: Option<&'a str>,
    pub preview: &'a str,
    pub cwd: &'a str,
    pub created_at: i64,
    pub updated_at: i64,
}
