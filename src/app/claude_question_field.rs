//! Keeps one native answer field separate from Carlos's message composer.

use crossterm::event::KeyEvent;
use ratatui_textarea::TextArea;
use serde_json::{json, Value};

use super::render::textarea_input_from_key;

/// One choice retains its native value while Carlos displays its label.
pub(super) struct AnswerOption {
    pub(super) value: Value,
    pub(super) label: String,
    pub(super) description: String,
}

/// One question owns its editor, cursor, selected choices, and omitted state.
pub(super) struct AnswerField {
    pub(super) name: String,
    pub(super) secret: bool,
    pub(super) title: String,
    pub(super) description: String,
    pub(super) options: Vec<AnswerOption>,
    pub(super) multiple: bool,
    pub(super) other: bool,
    pub(super) notes_only: bool,
    pub(super) required: bool,
    pub(super) omitted: bool,
    pub(super) cursor: usize,
    pub(super) selected: Vec<usize>,
    pub(super) use_text: bool,
    pub(super) editor: TextArea<'static>,
    pub(super) schema: Value,
}

impl AnswerField {
    /// Start an empty field without inserting anything into the main composer.
    pub(super) fn new(name: &str, schema: Value, required: bool) -> Self {
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
            omitted: !required,
            cursor: 0,
            selected: Vec::new(),
            use_text: false,
            editor: TextArea::default(),
            schema,
        }
    }

    /// Apply a supplied default to the editor or native choice values.
    pub(super) fn apply_default(&mut self) {
        let Some(value) = self.schema.get("default") else {
            return;
        };
        self.omitted = false;
        if self.options.is_empty() {
            let text = value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string());
            self.editor.insert_str(text);
        } else {
            self.selected = self
                .options
                .iter()
                .enumerate()
                .filter_map(|(i, option)| {
                    (value == &option.value
                        || value.as_array().is_some_and(|a| a.contains(&option.value)))
                    .then_some(i)
                })
                .collect();
        }
    }

    /// Choose or toggle the highlighted native value, including free text.
    pub(super) fn select(&mut self) {
        self.omitted = false;
        if self.other && self.cursor == self.options.len() {
            self.use_text = true;
            if !self.multiple && !self.notes_only {
                self.selected.clear();
            }
            return;
        }
        if self.cursor >= self.options.len() {
            return;
        }
        if !self.multiple {
            self.selected = vec![self.cursor];
            self.use_text = false;
        } else if self.selected.contains(&self.cursor) {
            self.selected.retain(|index| *index != self.cursor);
        } else {
            self.selected.push(self.cursor);
            self.selected.sort_unstable();
        }
    }

    /// Keep Unicode editing and cursor movement in this question's editor.
    pub(super) fn edit(&mut self, key: KeyEvent) {
        self.omitted = false;
        self.editor.input(textarea_input_from_key(key));
    }

    /// Insert pasted text into this field only.
    pub(super) fn paste(&mut self, text: &str) {
        if self.options.is_empty() || (self.other && self.cursor == self.options.len()) {
            self.omitted = false;
            self.use_text = self.other;
            if self.other && !self.multiple && !self.notes_only {
                self.selected.clear();
            }
            self.editor.insert_str(text);
        }
    }

    /// Read a typed MCP value; the helper checks the complete native schema.
    pub(super) fn mcp_value(&self) -> Result<Option<Value>, String> {
        if self.omitted && !self.required {
            return Ok(None);
        }
        if self.multiple {
            return Ok(Some(Value::Array(
                self.selected
                    .iter()
                    .map(|i| self.options[*i].value.clone())
                    .collect(),
            )));
        }
        if !self.options.is_empty() {
            return self
                .selected
                .first()
                .map(|i| Some(self.options[*i].value.clone()))
                .ok_or_else(|| format!("{}: choose a value", self.title));
        }
        let text = self.editor.lines().join("\n");
        let value = match self.schema["type"].as_str() {
            Some("string") => json!(text),
            Some("number" | "integer") => serde_json::from_str::<Value>(&text)
                .ok()
                .filter(Value::is_number)
                .ok_or_else(|| format!("{}: enter a number", self.title))?,
            _ => return Err(format!("{}: unsupported field", self.title)),
        };
        Ok(Some(value))
    }

    /// Return selected labels and custom text in the SDK's answer string format.
    pub(super) fn ask_value(&self) -> Result<String, String> {
        let mut labels: Vec<String> = self
            .selected
            .iter()
            .map(|i| self.options[*i].label.clone())
            .collect();
        if self.use_text {
            let text = self.editor.lines().join("\n");
            if !text.trim().is_empty() {
                labels.push(text);
            }
        }
        if labels.is_empty() {
            return Err(format!(
                "{}: choose an answer or enter Other text",
                self.title
            ));
        }
        Ok(labels.join(", "))
    }
}
