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
fn test_process_env_vars_strips_quotes() {
    unsafe {
        env::set_var("QUOTE_DOUBLE", "\"hello\"");
        env::set_var("QUOTE_SINGLE", "'world'");
        env::set_var("QUOTE_UNTERMINATED", "\"unterminated");

        let result = process_env_vars("QUOTE_").unwrap();

        assert_eq!(result["QUOTE_DOUBLE"].value, "hello");
        assert_eq!(result["QUOTE_SINGLE"].value, "world");
        assert_eq!(result["QUOTE_UNTERMINATED"].value, "\"unterminated");

        env::remove_var("QUOTE_DOUBLE");
        env::remove_var("QUOTE_SINGLE");
        env::remove_var("QUOTE_UNTERMINATED");
    }
}

#[test]
fn test_process_env_vars_ignores_non_matching_prefix() {
    unsafe {
        env::set_var("MATCHPREFIX_FOO", "matched");
        env::set_var("OTHERPREFIX_FOO", "unmatched");

        let result = process_env_vars("MATCHPREFIX_").unwrap();

        assert_eq!(result.len(), 1);
        assert!(result.contains_key("MATCHPREFIX_FOO"));
        assert!(!result.contains_key("OTHERPREFIX_FOO"));

        env::remove_var("MATCHPREFIX_FOO");
        env::remove_var("OTHERPREFIX_FOO");
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
fn test_create_nested_json_array_order_independent() {
    // Regression test: array elements must end up in the right slot
    // regardless of the order they're processed in (env vars are read
    // from a HashMap, whose iteration order is not guaranteed).
    let mut forward = Map::new();
    create_nested_json(&mut forward, "arr.0", "a");
    create_nested_json(&mut forward, "arr.1", "b");
    create_nested_json(&mut forward, "arr.2", "c");

    let mut reverse = Map::new();
    create_nested_json(&mut reverse, "arr.2", "c");
    create_nested_json(&mut reverse, "arr.1", "b");
    create_nested_json(&mut reverse, "arr.0", "a");

    let expected = json!({ "arr": ["a", "b", "c"] });

    assert_eq!(Value::Object(forward.clone()), expected);
    assert_eq!(Value::Object(reverse.clone()), expected);
    assert_eq!(Value::Object(forward), Value::Object(reverse));
}

#[test]
fn test_create_nested_json_top_level_scalar() {
    let mut config = Map::new();

    create_nested_json(&mut config, "key", "value");

    assert_eq!(Value::Object(config), json!({"key": "value"}));
}

#[test]
fn test_create_nested_json_array_of_objects_multiple_keys() {
    let mut config = Map::new();

    create_nested_json(&mut config, "servers.0.name", "alpha");
    create_nested_json(&mut config, "servers.0.port", "8080");
    create_nested_json(&mut config, "servers.1.name", "beta");
    create_nested_json(&mut config, "servers.1.port", "9090");

    let expected = json!({
        "servers": [
            {"name": "alpha", "port": "8080"},
            {"name": "beta", "port": "9090"}
        ]
    });

    assert_eq!(Value::Object(config), expected);
}

#[test]
fn test_create_nested_json_overwrite_existing_value() {
    let mut config = Map::new();

    create_nested_json(&mut config, "key", "first");
    create_nested_json(&mut config, "key", "second");

    assert_eq!(Value::Object(config), json!({"key": "second"}));
}

#[test]
fn test_fix_and_validate_json_already_valid() {
    let schema = json!({
        "type": "object",
        "properties": {
            "count": {"type": "number"}
        }
    });

    let mut config = Map::new();
    config.insert("count".to_string(), Value::Number(42.into()));

    let result = fix_and_validate_json(&schema, config.clone(), false).unwrap();

    assert_eq!(Value::Object(result), Value::Object(config));
}

#[test]
fn test_fix_and_validate_json_nested_object() {
    let schema = json!({
        "type": "object",
        "properties": {
            "database": {
                "type": "object",
                "properties": {
                    "port": {"type": "number"}
                }
            }
        }
    });

    let mut db = Map::new();
    db.insert("port".to_string(), Value::String("5432".to_string()));
    let mut config = Map::new();
    config.insert("database".to_string(), Value::Object(db));

    let result = fix_and_validate_json(&schema, config, false).unwrap();

    assert_eq!(result["database"]["port"], json!(5432));
}

#[test]
fn test_fix_and_validate_json_array_of_objects() {
    let schema = json!({
        "type": "object",
        "properties": {
            "servers": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "port": {"type": "number"}
                    }
                }
            }
        }
    });

    let mut server = Map::new();
    server.insert("port".to_string(), Value::String("8080".to_string()));
    let mut config = Map::new();
    config.insert(
        "servers".to_string(),
        Value::Array(vec![Value::Object(server)]),
    );

    let result = fix_and_validate_json(&schema, config, false).unwrap();

    assert_eq!(result["servers"][0]["port"], json!(8080));
}

#[test]
fn test_fix_and_validate_json_retried_returns_error_string() {
    let schema = json!({
        "type": "object",
        "properties": {
            "count": {"type": "number"}
        }
    });

    let mut config = Map::new();
    config.insert(
        "count".to_string(),
        Value::String("not-a-number".to_string()),
    );

    let result = fix_and_validate_json(&schema, config, true);

    assert!(result.is_err());
    assert!(!result.unwrap_err().is_empty());
}

#[test]
fn test_fix_and_validate_json_unfixable_value_returns_err() {
    // Regression test: a value that can't be coerced into the target
    // primitive type must return an Err, not panic.
    let schema = json!({
        "type": "object",
        "properties": {
            "enabled": {"type": "boolean"}
        }
    });

    let mut config = Map::new();
    config.insert(
        "enabled".to_string(),
        Value::String("not-a-bool".to_string()),
    );

    let result = fix_and_validate_json(&schema, config, false);

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Boolean"));
}

#[test]
fn test_fix_and_validate_json_multiple_allowed_types_returns_err() {
    let schema = json!({
        "type": "object",
        "properties": {
            "flexible": {"type": ["boolean", "number"]}
        }
    });

    let mut config = Map::new();
    config.insert("flexible".to_string(), Value::Array(vec![]));

    let result = fix_and_validate_json(&schema, config, false);

    assert!(result.is_err());
}

#[test]
fn test_fix_and_validate_json_invalid_schema_returns_err() {
    let schema = json!({"type": 123});
    let config = Map::new();

    let result = fix_and_validate_json(&schema, config, false);

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Failed to compile schema"));
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

#[test]
fn test_resolve_ref_without_hash_prefix() {
    let schema = json!({
        "definitions": {
            "address": {"type": "object"}
        }
    });

    let result = resolve_ref(&schema, "definitions/address").unwrap();

    assert_eq!(result, &json!({"type": "object"}));
}

#[test]
fn test_resolve_ref_deep_path() {
    let schema = json!({
        "definitions": {
            "address": {
                "properties": {
                    "street": {"type": "string"}
                }
            }
        }
    });

    let result = resolve_ref(&schema, "#/definitions/address/properties/street").unwrap();

    assert_eq!(result, &json!({"type": "string"}));
}
