//! Keep one neutral form description across terminal and native clients.

use serde_json::json;

use crate::forms::{fields, FormError};

#[test]
fn typed_options_keep_labels_values_and_constraints_without_applying_defaults() {
    let schema = json!({"type":"object","required":["mode"],"properties":{
        "mode":{"type":"string","oneOf":[{"const":"jp","title":"日本語"},{"const":"fr","title":"Français"}],"default":"jp"},
        "count":{"type":"integer","minimum":1,"maximum":9,"default":3},
        "enabled":{"type":"boolean","default":false},
        "optional":{"type":"string"}}});
    let fields = fields(&schema).expect("flat form");
    let mode = fields
        .iter()
        .find(|field| field.key == "mode")
        .expect("mode");
    assert!(mode.required);
    assert_eq!(mode.options[0].value, json!("jp"));
    assert_eq!(mode.options[0].label, "日本語");
    assert_eq!(mode.schema, schema["properties"]["mode"]);
    let boolean = fields
        .iter()
        .find(|field| field.key == "enabled")
        .expect("boolean");
    assert_eq!(boolean.options[1].value, json!(false));
    assert!(
        !fields
            .iter()
            .find(|field| field.key == "optional")
            .expect("optional")
            .required
    );
}

#[test]
fn array_choices_and_legacy_enum_titles_keep_exact_original_values() {
    let fields = fields(&json!({"properties":{
        "many":{"type":"array","items":{"enum":["a,b","🌍"],"enumNames":["First","Second"]}},
        "number":{"type":"integer","enum":[17],"enumNames":["Named integer"]}
    }}))
    .expect("named choices");
    let multiple = fields
        .iter()
        .find(|field| field.key == "many")
        .expect("array");
    assert!(multiple.multiple);
    assert_eq!(multiple.options[0].value, json!("a,b"));
    assert_eq!(
        fields
            .iter()
            .find(|field| field.key == "number")
            .expect("numeric choice")
            .options[0]
            .value,
        json!(17)
    );
}

#[test]
fn malformed_descriptions_return_a_typed_error_without_echoing_values() {
    assert!(matches!(
        fields(&json!({})),
        Err(FormError::MissingProperties)
    ));
    assert!(matches!(
        fields(&json!({"properties":{"x":{"enum":[17]}}})),
        Err(FormError::NonStringChoice)
    ));
    let properties: serde_json::Map<_, _> = (0..65)
        .map(|index| (format!("field-{index}"), json!({"type":"string"})))
        .collect();
    assert!(matches!(
        fields(&json!({"properties":properties})),
        Err(FormError::TooManyFields)
    ));
}
