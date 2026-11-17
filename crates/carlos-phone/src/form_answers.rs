//! Build typed flat form content from captured choices and exact field text, without validating native constraints twice.

use carlos_proto::session::InputField;
use serde_json::{Map, Value};

use crate::approval_inputs::{bounded, invalid, selected_values};
use crate::{PhoneError, PhoneQuestionAnswer};

pub(crate) fn encode(fields: Vec<(&InputField, PhoneQuestionAnswer)>) -> Result<Value, PhoneError> {
    let mut content = Map::new();
    for (field, answer) in fields {
        let value = if !field.options.is_empty() {
            choice(field, answer)?
        } else {
            scalar(field, answer)?
        };
        content.insert(field.key.clone(), value);
    }
    bounded(Value::Object(content))
}

fn choice(field: &InputField, answer: PhoneQuestionAnswer) -> Result<Value, PhoneError> {
    if answer.text.as_ref().is_some_and(|text| !text.is_empty()) {
        return Err(invalid("Choose an offered value for this field"));
    }
    let values = selected_values(field, answer.selected)?;
    if field.multiple {
        return Ok(Value::Array(values));
    }
    values
        .into_iter()
        .next()
        .ok_or_else(|| invalid("Choose a value for each included field"))
}

fn scalar(field: &InputField, answer: PhoneQuestionAnswer) -> Result<Value, PhoneError> {
    if !answer.selected.is_empty() {
        return Err(invalid("This field accepts text, not a listed choice"));
    }
    let text = answer.text.unwrap_or_default();
    match field.schema["type"].as_str() {
        Some("string") => Ok(Value::String(text)),
        Some("number" | "integer") => number(text.trim(), field.schema["type"] == "integer"),
        _ => Err(invalid("This form field cannot be edited on this device")),
    }
}

fn number(text: &str, integer: bool) -> Result<Value, PhoneError> {
    if let Ok(value) = text.parse::<i64>() {
        return Ok(Value::from(value));
    }
    if let Ok(value) = text.parse::<u64>() {
        return Ok(Value::from(value));
    }
    // An overflowing integer must not fall through to a rounded floating-point value.
    if integer {
        return Err(invalid(
            "Enter a whole number within the supported 64-bit range",
        ));
    }
    text.parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map(Value::Number)
        .ok_or_else(|| invalid("Enter a number using a dot for decimals"))
}
