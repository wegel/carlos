//! Explain failed form rules without echoing any submitted answer values.

use serde_json::Value;

/// Name the expected rule and its native limit rather than printing the bad value.
pub(super) fn describe(schema: &Value, answer: &Value, path: &str, rule_path: &str) -> String {
    let key = path
        .strip_prefix('/')
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("")
        .replace("~1", "/")
        .replace("~0", "~");
    let rule = &schema["properties"][&key];
    let label = rule["title"]
        .as_str()
        .unwrap_or(if key.is_empty() { "Form" } else { &key });
    let keyword = rule_path.rsplit('/').next().unwrap_or("");
    let advice = match keyword {
        "minimum" => format!("enter at least {}", rule["minimum"]),
        "maximum" => format!("enter at most {}", rule["maximum"]),
        "minLength" => format!("enter at least {} characters", rule["minLength"]),
        "maxLength" => format!("enter at most {} characters", rule["maxLength"]),
        "minItems" => format!("choose at least {} values", rule["minItems"]),
        "maxItems" => format!("choose at most {} values", rule["maxItems"]),
        "type" => match rule["type"].as_str() {
            Some("integer") => "enter a whole number".into(),
            Some("number") => "enter a finite number".into(),
            Some("boolean") => "choose true or false".into(),
            Some("array") => "choose the listed values".into(),
            _ => "enter text".into(),
        },
        "format" => match rule["format"].as_str() {
            Some("email") => "enter a valid email address".into(),
            Some("uri") => "enter a valid absolute address".into(),
            Some("date") => "enter a date as YYYY-MM-DD".into(),
            Some("date-time") => "enter a valid date and time with a timezone".into(),
            _ => "use the requested format".into(),
        },
        "pattern" => "match the pattern shown for this field".into(),
        "enum" | "const" | "oneOf" | "anyOf" => "choose the listed values".into(),
        "additionalProperties" => "remove unrecognized fields".into(),
        "required" => return missing_field(schema, answer),
        _ => "check the constraints shown for this field".into(),
    };
    format!("{label}: {advice}")
}

fn missing_field(schema: &Value, answer: &Value) -> String {
    for field in schema["required"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if answer.get(field).is_none() {
            let title = schema["properties"][field]["title"]
                .as_str()
                .unwrap_or(field);
            return format!("{title}: answer this required field");
        }
    }
    "Answer every required field".into()
}
