use env_to_schema_json::{
    create_nested_json, fix_and_validate_json, process_env_vars, resolve_ref,
};
use serde_json::{Map, Value, json};
use std::env;

#[test]
fn test_process_env_vars() {
    unsafe {
        env::set_var("TEST_FOO_BAR", "value1");
        env::set_var("TEST_BAZ__QUX", "value2");

        let result = process_env_vars("TEST_").unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result["TEST_FOO_BAR"].path, "foo.bar");
        assert_eq!(result["TEST_FOO_BAR"].value, "value1");
        assert_eq!(result["TEST_BAZ__QUX"].path, "baz_qux");
        assert_eq!(result["TEST_BAZ__QUX"].value, "value2");

        env::remove_var("TEST_FOO_BAR");
        env::remove_var("TEST_BAZ__QUX");
    }
}

#[test]
fn test_create_nested_json() {
    let mut config = Map::new();

    create_nested_json(&mut config, "a.b.0.c", "value1");
    create_nested_json(&mut config, "a.b.1", "value2");

    let expected = json!({
        "a": {
            "b": [
                {"c": "value1"},
                "value2"
            ]
        }
    });

    assert_eq!(Value::Object(config), expected);
}

#[test]
fn test_fix_and_validate_json() {
    let schema = json!({
        "type": "object",
        "properties": {
            "string": {"type": "string"},
            "number": {"type": "number"},
            "boolean_true": {"type": "boolean"},
            "boolean_false": {"type": "boolean"},
            "array": {"type": "array", "items": {"type": "string"}}
        }
    });

    let mut config = Map::new();
    config.insert("string".to_string(), Value::String("string".to_string()));
    config.insert("number".to_string(), Value::String("42".to_string()));
    config.insert(
        "boolean_true".to_string(),
        Value::String("true".to_string()),
    );
    config.insert(
        "boolean_false".to_string(),
        Value::String("false".to_string()),
    );
    config.insert("array".to_string(), Value::String("1, 2, 3".to_string()));

    let result = fix_and_validate_json(&schema, config, false).unwrap();

    assert_eq!(result["string"], json!("string"));
    assert_eq!(result["number"], json!(42));
    assert_eq!(result["boolean_true"], json!(true));
    assert_eq!(result["boolean_false"], json!(false));
    assert_eq!(result["array"], json!(vec!["1", "2", "3"]));
}

#[test]
fn test_resolve_ref() {
    let schema = json!({
        "definitions": {
            "address": {
                "type": "object",
                "properties": {
                    "street": {"type": "string"}
                }
            }
        }
    });

    let result = resolve_ref(&schema, "#/definitions/address").unwrap();
    let expected = json!({
        "type": "object",
        "properties": {
            "street": {"type": "string"}
        }
    });

    assert_eq!(result, &expected);
    assert!(resolve_ref(&schema, "#/invalid/path").is_none());
}
