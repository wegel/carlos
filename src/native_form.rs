//! Validate standard MCP forms locally with no file or network schema retrieval.

#[path = "native_form_errors.rs"]
mod errors;

use std::sync::OnceLock;

use anyhow::{bail, ensure, Context, Result};
use jsonschema::{Draft, EmailOptions, PatternOptions, Retrieve, Uri, Validator};
use serde_json::{json, Value};

struct NoRetrieval;

impl Retrieve for NoRetrieval {
    fn retrieve(
        &self,
        _uri: &Uri<String>,
    ) -> std::result::Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err("External schema retrieval is disabled".into())
    }
}

/// Compile a complete supported flat form, rejecting unknown fields and constraints.
pub(crate) fn compile(schema: &Value) -> Result<Validator> {
    ensure!(
        schema.to_string().len() <= 65536,
        "Form schema exceeds 64 KiB"
    );
    let properties = schema["properties"]
        .as_object()
        .context("Form properties are missing")?;
    ensure!(properties.len() <= 64, "Form exceeds 64 fields");
    let meta = meta_schema();
    let meta = meta
        .as_ref()
        .map_err(|error| anyhow::anyhow!(error.clone()))?;
    if let Err(error) = meta.validate(schema) {
        bail!("Unsupported form schema at {}", error.instance_path());
    }
    let mut required = std::collections::HashSet::new();
    for name in schema["required"].as_array().into_iter().flatten() {
        let name = name.as_str().context("Required field name is invalid")?;
        ensure!(
            properties.contains_key(name) && required.insert(name),
            "Required fields must name distinct existing properties"
        );
    }
    let schema = normalize(schema)?;
    let validator = options().build(&schema).map_err(|_| {
        anyhow::anyhow!("Invalid or unsupported form constraint or regular expression")
    })?;
    Ok(validator)
}

/// Validate typed content without including answer values in a diagnostic.
pub(crate) fn validate(schema: &Value, answer: &Value) -> Result<()> {
    ensure!(
        answer.to_string().len() <= 65536,
        "Form answer exceeds 64 KiB"
    );
    if let Err(error) = compile(schema)?.validate(answer) {
        bail!(
            "{}",
            errors::describe(
                schema,
                answer,
                error.instance_path().as_str(),
                error.schema_path().as_str()
            )
        );
    }
    Ok(())
}

fn options() -> jsonschema::ValidationOptions<'static, std::sync::Arc<dyn Retrieve>> {
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_retriever(NoRetrieval)
        .should_validate_formats(true)
        .with_email_options(
            EmailOptions::default()
                .with_no_minimum_sub_domains()
                .without_display_text(),
        )
        .with_pattern_options(
            PatternOptions::regex()
                .size_limit(1024 * 1024)
                .dfa_size_limit(1024 * 1024),
        )
}

fn meta_schema() -> &'static std::result::Result<Validator, String> {
    static META: OnceLock<std::result::Result<Validator, String>> = OnceLock::new();
    META.get_or_init(|| {
        let native:Value = serde_json::from_str(include_str!("../docs/app-server-schema/McpServerElicitationRequestParams.json")).expect("native MCP schema");
        let mut schema=json!({"$ref":"#/definitions/McpElicitationSchema","definitions":native["definitions"]});
        // Accept this safe additive constraint too; the native typed schema may omit it.
        schema["definitions"]["McpElicitationStringSchema"]["properties"]["pattern"] = json!({"type":"string","maxLength":4096});
        options().build(&schema).map_err(|_|"Could not compile the native MCP schema".into())
    })
}

fn normalize(schema: &Value) -> Result<Value> {
    let mut normalized = schema.clone();
    let root = normalized
        .as_object_mut()
        .context("Expected a form object")?;
    if let Some(dialect) = root.remove("$schema").filter(|value| !value.is_null()) {
        ensure!(
            matches!(
                dialect.as_str(),
                Some(
                    "https://json-schema.org/draft/2020-12/schema"
                        | "https://json-schema.org/draft/2019-09/schema"
                        | "http://json-schema.org/draft-07/schema#"
                )
            ),
            "Unsupported schema dialect"
        );
    }
    if root.get("required").is_some_and(Value::is_null) {
        root.remove("required");
    }
    let properties = root
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .expect("checked properties");
    for (name, field) in properties {
        let field = field.as_object_mut().context("Expected a field object")?;
        field.retain(|_, value| !value.is_null());
        for (min, max) in [
            ("minLength", "maxLength"),
            ("minItems", "maxItems"),
            ("minimum", "maximum"),
        ] {
            if let (Some(min), Some(max)) = (
                field.get(min).and_then(Value::as_f64),
                field.get(max).and_then(Value::as_f64),
            ) {
                ensure!(min <= max, "{name}: minimum exceeds maximum");
            }
        }
        check_choices(field, name)?;
    }
    root.insert("additionalProperties".into(), json!(false));
    Ok(normalized)
}

fn check_choices(field: &serde_json::Map<String, Value>, name: &str) -> Result<()> {
    let choices = field
        .get("items")
        .and_then(Value::as_object)
        .unwrap_or(field);
    let constraint = choices
        .get("enum")
        .or_else(|| choices.get("oneOf"))
        .or_else(|| choices.get("anyOf"));
    if let Some(options) = constraint.and_then(Value::as_array) {
        ensure!(!options.is_empty(), "{name}: choices are empty");
        let values = options
            .iter()
            .map(|option| option.get("const").unwrap_or(option))
            .collect::<Vec<_>>();
        for (index, value) in values.iter().enumerate() {
            ensure!(
                !values[..index].contains(value),
                "{name}: choices repeat a value"
            );
        }
        if let Some(labels) = choices.get("enumNames").and_then(Value::as_array) {
            ensure!(
                labels.len() == values.len(),
                "{name}: choice titles do not match values"
            );
        }
    }
    Ok(())
}

/// Check an HTTP(S) consent link without opening it or normalizing away bad bytes.
pub(crate) fn url_host(value: &str) -> Result<String> {
    ensure!(value.len() <= 8192, "Browser link exceeds 8192 bytes");
    ensure!(
        !value
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control()),
        "Browser link contains whitespace or control characters"
    );
    let (_, authority) = value
        .split_once("://")
        .context("Browser link must be an absolute HTTP(S) URL")?;
    ensure!(
        !authority.is_empty() && !authority.starts_with(['/', '?', '#']),
        "Browser link has no host"
    );
    let url = url::Url::parse(value).context("Invalid browser link")?;
    ensure!(
        matches!(url.scheme(), "http" | "https"),
        "Browser link must use HTTP or HTTPS"
    );
    Ok(url
        .host_str()
        .filter(|host| !host.is_empty())
        .context("Browser link has no host")?
        .to_string())
}

#[cfg(test)]
#[path = "tests/native_form_tests.rs"]
mod tests;
