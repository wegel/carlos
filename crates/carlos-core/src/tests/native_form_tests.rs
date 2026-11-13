//! Check complete standard MCP schemas, formats and bounded local regex work.

use std::time::{Duration, Instant};

use serde_json::json;

use super::{compile, url_host, validate};

#[test]
fn flat_forms_keep_types_unicode_optional_omissions_and_native_titled_enums() {
    let schema = json!({"type":"object","required":["count","choice","many"],"properties":{
        "count":{"type":"integer","minimum":0,"maximum":10},
        "choice":{"type":"string","oneOf":[{"const":"jp","title":"日本語"},{"const":"fr","title":"Français"}]},
        "many":{"type":"array","items":{"anyOf":[{"const":"a","title":"Alpha"},{"const":"b","title":"Beta"}]},"minItems":1,"maxItems":2},
        "optional":{"type":"boolean","default":false},"text":{"type":"string","minLength":2,"maxLength":3}}});
    validate(
        &schema,
        &json!({"count":2,"choice":"jp","many":["a"],"text":"日本語"}),
    )
    .expect("typed form");
    for bad in [
        json!({"count":"2","choice":"jp","many":["a"]}),
        json!({"count":2,"choice":"jp","many":[]}),
        json!({"count":2,"choice":"missing","many":["a"]}),
        json!({"count":2,"choice":"jp","many":["a"],"unknown":true}),
    ] {
        assert!(validate(&schema, &bad).is_err());
    }
}

#[test]
fn unsupported_unknown_and_inconsistent_schemas_fail_before_accept() {
    for schema in [
        json!({"type":"object","properties":{},"required":["missing"]}),
        json!({"type":"object","properties":{"x":{"type":"string"}},"required":["x","x"]}),
        json!({"type":"object","properties":{"x":{"type":"string","minLength":4,"maxLength":2}}}),
        json!({"type":"object","properties":{"x":{"type":"string","enum":["a","a"]}}}),
        json!({"type":"object","properties":{"x":{"type":"string","enum":["a"],"enumNames":["A","B"]}}}),
        json!({"type":"object","properties":{"x":{"type":"string","unknownConstraint":true}}}),
        json!({"type":"object","properties":{"x":{"type":"object","properties":{}}}}),
        json!({"type":"object","$ref":"https://example.invalid/private","properties":{}}),
        json!({"type":"object","properties":{"x":{"type":"string","enum":["a"],"oneOf":[{"const":"a","title":"A"}]}}}),
    ] {
        assert!(compile(&schema).is_err(), "must reject {schema}");
    }
}

#[test]
fn native_null_optional_metadata_is_absent_instead_of_becoming_a_null_constraint() {
    let schema = json!({"type":"object","$schema":null,"required":null,"properties":{"name":{"type":"string","default":null,"title":null,"minLength":null,"format":null}}});
    validate(&schema, &json!({"name":"日本語"})).expect("native optional nulls");
    validate(&schema, &json!({})).expect("optional omission");
}

#[test]
fn maintained_format_checks_accept_valid_edges_and_reject_invalid_values() {
    for (format, good, bad) in [
        ("email", "a@b", "not-an-email"),
        (
            "uri",
            "https://example.test/path",
            "https://example.test/a b",
        ),
        ("date", "2024-02-29", "2023-02-29"),
        ("date-time", "1990-12-31T23:59:60Z", "2026-19-40T25:99:99Z"),
    ] {
        let schema = json!({"type":"object","properties":{"x":{"type":"string","format":format}}});
        validate(&schema, &json!({"x":good}))
            .unwrap_or_else(|error| panic!("{format} valid edge failed: {error}"));
        assert!(
            validate(&schema, &json!({"x":bad})).is_err(),
            "invalid {format}"
        );
    }
}

#[test]
fn regex_work_is_linear_and_failures_do_not_quote_answer_values() {
    let schema = json!({"type":"object","properties":{"x":{"type":"string","pattern":"^(a+)+$"}}});
    let started = Instant::now();
    let value = format!("{}secret-suffix", "a".repeat(60000));
    let error = validate(&schema, &json!({"x":value})).expect_err("nonmatching pattern");
    assert!(!error.to_string().contains("secret-suffix"));
    assert!(started.elapsed() < Duration::from_secs(1));
    let unsupported =
        json!({"type":"object","properties":{"x":{"type":"string","pattern":"(?<=a)b"}}});
    assert!(
        compile(&unsupported).is_err(),
        "advanced regex is explicitly unsupported"
    );
}

#[test]
fn codex_integer_answers_keep_exact_native_rust_precision() {
    let schema = json!({"type":"object","properties":{"n":{"type":"integer"}}});
    let value = json!({"n":9007199254740993u64});
    validate(&schema, &value).expect("no JavaScript bridge");
    assert_eq!(value["n"].as_u64(), Some(9007199254740993));
}

#[test]
fn browser_consent_uses_an_actual_http_host_and_rejects_bad_links() {
    assert_eq!(
        url_host("https://user@real.example/path").expect("valid host"),
        "real.example"
    );
    assert_eq!(
        url_host("HTTPS://%72eal.example/path").expect("canonical host"),
        "real.example"
    );
    assert_eq!(
        url_host("https://例子.test/path").expect("international host"),
        "xn--fsqu00a.test"
    );
    for bad in [
        "file:///tmp/a",
        "javascript:alert(1)",
        "https:///path",
        "https://example.test/a b",
        "https://example.test/\nnext",
    ] {
        assert!(url_host(bad).is_err(), "invalid link {bad:?}");
    }
}

#[test]
fn answer_errors_show_expected_limits_types_and_formats_without_secret_values() {
    let schema = json!({"type":"object","properties":{"n":{"title":"Count","type":"integer","minimum":1,"maximum":10}}});
    assert_eq!(
        validate(&schema, &json!({"n":11}))
            .expect_err("maximum")
            .to_string(),
        "Count: enter at most 10"
    );
    let error = validate(&schema, &json!({"n":"PRIVATE_SECRET"}))
        .expect_err("type")
        .to_string();
    assert_eq!(error, "Count: enter a whole number");
    assert!(!error.contains("PRIVATE_SECRET"));
}
