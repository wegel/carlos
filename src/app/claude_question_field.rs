//! Keeps one native answer field separate from Carlos's message composer.

use carlos_core::session::questions::QuestionField;
use crossterm::event::KeyEvent;
use ratatui_textarea::TextArea;
use serde_json::{json, Value};

use super::render::textarea_input_from_key;

/// One question owns its editor, cursor, selected choices, and omitted state.
pub(super) struct AnswerField {
    pub(super) spec: QuestionField,
    pub(super) omitted: bool,
    pub(super) cursor: usize,
    pub(super) selected: Vec<usize>,
    pub(super) use_text: bool,
    pub(super) editor: TextArea<'static>,
}

impl AnswerField {
    /// Start an empty editor for one parsed native field.
    pub(super) fn from_description(spec: QuestionField) -> Self {
        Self {
            omitted: !spec.required(),
            cursor: 0,
            selected: Vec::new(),
            use_text: false,
            editor: TextArea::default(),
            spec,
        }
    }

    /// Apply a supplied default to the editor or native choice values.
    pub(super) fn apply_default(&mut self) {
        let Some(value) = self.spec.schema().get("default") else {
            return;
        };
        self.omitted = false;
        if self.spec.options().is_empty() {
            let text = value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string());
            self.editor.insert_str(text);
        } else {
            self.selected = self
                .spec
                .options()
                .iter()
                .enumerate()
                .filter_map(|(i, option)| {
                    (value == option.value()
                        || value.as_array().is_some_and(|a| a.contains(option.value())))
                    .then_some(i)
                })
                .collect();
        }
    }

    /// Choose or toggle the highlighted native value, including free text.
    pub(super) fn select(&mut self) {
        self.omitted = false;
        if self.spec.other() && self.cursor == self.spec.options().len() {
            self.use_text = true;
            if !self.spec.multiple() && !self.spec.notes_only() {
                self.selected.clear();
            }
            return;
        }
        if self.cursor >= self.spec.options().len() {
            return;
        }
        if !self.spec.multiple() {
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
        if self.spec.options().is_empty()
            || (self.spec.other() && self.cursor == self.spec.options().len())
        {
            self.omitted = false;
            self.use_text = self.spec.other();
            if self.spec.other() && !self.spec.multiple() && !self.spec.notes_only() {
                self.selected.clear();
            }
            self.editor.insert_str(text);
        }
    }

    /// Read a typed MCP value; the helper checks the complete native schema.
    pub(super) fn mcp_value(&self) -> Result<Option<Value>, String> {
        if self.omitted && !self.spec.required() {
            return Ok(None);
        }
        if self.spec.multiple() {
            return Ok(Some(Value::Array(
                self.selected
                    .iter()
                    .map(|i| self.spec.options()[*i].value().clone())
                    .collect(),
            )));
        }
        if !self.spec.options().is_empty() {
            return self
                .selected
                .first()
                .map(|i| Some(self.spec.options()[*i].value().clone()))
                .ok_or_else(|| format!("{}: choose a value", self.spec.title()));
        }
        let text = self.editor.lines().join("\n");
        let value = match self.spec.schema()["type"].as_str() {
            Some("string") => json!(text),
            Some("number" | "integer") => serde_json::from_str::<Value>(&text)
                .ok()
                .filter(Value::is_number)
                .ok_or_else(|| format!("{}: enter a number", self.spec.title()))?,
            _ => return Err(format!("{}: unsupported field", self.spec.title())),
        };
        Ok(Some(value))
    }

    /// Return selected labels and custom text in the SDK's answer string format.
    pub(super) fn ask_value(&self) -> Result<String, String> {
        let mut labels: Vec<String> = self
            .selected
            .iter()
            .map(|i| self.spec.options()[*i].label().to_string())
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
                self.spec.title()
            ));
        }
        Ok(labels.join(", "))
    }
}
