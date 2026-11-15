//! Describe native answer fields without editor state.

use carlos_proto::session::InputField;
use serde_json::Value;

/// One choice retains its native value while Carlos displays its label.
pub struct AnswerOption {
    pub(in crate::session) value: Value,
    pub(in crate::session) label: String,
    pub(in crate::session) description: String,
}

/// A backend field describes its schema and choices before a user edits an answer.
pub struct QuestionField {
    pub(in crate::session) name: String,
    pub(in crate::session) secret: bool,
    pub(in crate::session) title: String,
    pub(in crate::session) description: String,
    pub(in crate::session) options: Vec<AnswerOption>,
    pub(in crate::session) multiple: bool,
    pub(in crate::session) other: bool,
    pub(in crate::session) notes_only: bool,
    pub(in crate::session) required: bool,
    pub(in crate::session) schema: Value,
}

impl QuestionField {
    /// Reuse the native field editor with one checked neutral daemon description.
    pub fn from_input_field(field: InputField) -> Self {
        let other = field.other && !field.options.is_empty();
        Self {
            name: field.key,
            secret: field.secret,
            title: field.title,
            description: field.description,
            multiple: field.multiple,
            other,
            notes_only: field.notes_only,
            required: field.required,
            schema: field.schema,
            options: field
                .options
                .into_iter()
                .map(|option| AnswerOption {
                    value: option.value,
                    label: option.label,
                    description: option.description,
                })
                .collect(),
        }
    }
    /// Describe one native field without constructing an answer editor.
    pub(in crate::session) fn new(name: &str, schema: Value, required: bool) -> Self {
        Self {
            name: name.to_string(),
            secret: false,
            title: schema["title"].as_str().unwrap_or(name).to_string(),
            description: schema["description"].as_str().unwrap_or("").to_string(),
            options: Vec::new(),
            multiple: schema["type"] == "array",
            other: false,
            notes_only: false,
            required,
            schema,
        }
    }

    /// Read the native field's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Report whether the native question hides typed text.
    pub fn secret(&self) -> bool {
        self.secret
    }

    /// Read the native field's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Read the native field's description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Read the native field's options.
    pub fn options(&self) -> &[AnswerOption] {
        &self.options
    }

    /// Report whether the field accepts several values.
    pub fn multiple(&self) -> bool {
        self.multiple
    }

    /// Report whether the field offers free text beside its choices.
    pub fn other(&self) -> bool {
        self.other
    }

    /// Report whether free text supplements a listed choice.
    pub fn notes_only(&self) -> bool {
        self.notes_only
    }

    /// Report whether the native server requires this field.
    pub fn required(&self) -> bool {
        self.required
    }

    /// Read the native field's schema.
    pub fn schema(&self) -> &Value {
        &self.schema
    }
}

impl AnswerOption {
    /// Read the choice's native value.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Read the choice's native label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Read the choice's native description.
    pub fn description(&self) -> &str {
        &self.description
    }
}
