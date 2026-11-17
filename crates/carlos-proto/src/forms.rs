//! Describe the protocol's flat form fields without applying defaults or validating answers.

use std::fmt;

use serde_json::{json, Value};

use crate::session::{InputField, InputOption};

/// A form cannot be presented as flat native fields without properties and choice labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormError {
    MissingProperties,
    TooManyFields,
    NonStringChoice,
}

/// Preserve original typed choices and field rules for terminal, phone and web editors.
pub fn fields(schema: &Value) -> Result<Vec<InputField>, FormError> {
    let properties = schema["properties"]
        .as_object()
        .ok_or(FormError::MissingProperties)?;
    if properties.len() > 64 {
        return Err(FormError::TooManyFields);
    }
    properties
        .iter()
        .map(|(name, rule)| field(schema, name, rule))
        .collect()
}

fn field(schema: &Value, name: &str, rule: &Value) -> Result<InputField, FormError> {
    let multiple = rule["type"] == "array";
    let options = if rule["type"] == "boolean" {
        [true, false]
            .into_iter()
            .map(|value| InputOption {
                value: json!(value),
                label: value.to_string(),
                description: String::new(),
            })
            .collect()
    } else {
        options(if multiple { &rule["items"] } else { rule })?
    };
    Ok(InputField {
        key: name.to_owned(),
        title: rule["title"].as_str().unwrap_or(name).to_owned(),
        description: rule["description"].as_str().unwrap_or("").to_owned(),
        secret: false,
        multiple,
        other: false,
        notes_only: false,
        required: schema["required"]
            .as_array()
            .is_some_and(|keys| keys.iter().any(|key| key == name)),
        schema: rule.clone(),
        options,
    })
}

fn options(rule: &Value) -> Result<Vec<InputOption>, FormError> {
    if let Some(options) = rule["enum"].as_array() {
        return options
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let label = rule["enumNames"][index]
                    .as_str()
                    .or(value.as_str())
                    .ok_or(FormError::NonStringChoice)?;
                Ok(InputOption {
                    value: value.clone(),
                    label: label.to_owned(),
                    description: String::new(),
                })
            })
            .collect();
    }
    let options = rule["oneOf"]
        .as_array()
        .or_else(|| rule["anyOf"].as_array());
    Ok(options
        .map(|options| {
            options
                .iter()
                .map(|option| InputOption {
                    value: option["const"].clone(),
                    label: option["title"].as_str().unwrap_or("").to_owned(),
                    description: String::new(),
                })
                .collect()
        })
        .unwrap_or_default())
}

impl fmt::Display for FormError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingProperties => "Missing form properties",
            Self::TooManyFields => "Form exceeds 64 fields",
            Self::NonStringChoice => "Non-string choice",
        })
    }
}

impl std::error::Error for FormError {}

#[cfg(test)]
#[path = "form_tests.rs"]
mod tests;
