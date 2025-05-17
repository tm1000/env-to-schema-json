use env_to_schema_json::{
    PropertyInfo, PropertyValue, get_properties, parse_value, process_env_vars,
};
use serde_json::json;
use std::collections::HashMap;
use std::env;

#[test]
fn test_get_properties_simple() {
    let schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "age": { "type": "integer" },
            "height": { "type": "number" },
            "is_active": { "type": "boolean" },
            "tags": {
                "type": "array",
                "items": { "type": "string" }
            }
        }
    });

    let properties = get_properties(&schema, &schema, "");

    assert_eq!(properties.len(), 5);
    assert!(
        properties
            .iter()
            .any(|p| p.path == "name" && p.property_type == "string")
    );
    assert!(
        properties
            .iter()
            .any(|p| p.path == "age" && p.property_type == "integer")
    );
    assert!(
        properties
            .iter()
            .any(|p| p.path == "height" && p.property_type == "number")
    );
    assert!(
        properties
            .iter()
            .any(|p| p.path == "is_active" && p.property_type == "boolean")
    );
    assert!(
        properties
            .iter()
            .any(|p| p.path == "tags" && p.property_type == "array[string]")
    );
}

#[test]
fn test_parse_value() {
    // Test string parsing
    let result = parse_value("hello", "string", "test.path");
    assert!(matches!(result.unwrap(), PropertyValue::String(s) if s == "hello"));

    // Test invalid string (looks like number)
    let result = parse_value("123", "string", "test.path");
    assert!(result.is_err());

    // Test number parsing
    let result = parse_value("123.45", "number", "test.path");
    assert!(matches!(result.unwrap(), PropertyValue::Number(n) if n == 123.45));

    // Test boolean parsing
    let result = parse_value("true", "boolean", "test.path");
    assert!(matches!(result.unwrap(), PropertyValue::Boolean(b) if b));

    // Test integer parsing
    let result = parse_value("42", "integer", "test.path");
    assert!(matches!(result.unwrap(), PropertyValue::Integer(i) if i == 42));

    // Test array parsing
    let result = parse_value("one two three", "array[string]", "test.path");
    assert!(
        matches!(result.unwrap(), PropertyValue::StringArray(arr) if arr == vec!["one", "two", "three"])
    );

    let result = parse_value("1.1 2.2 3.3", "array[number]", "test.path");
    assert!(
        matches!(result.unwrap(), PropertyValue::NumberArray(arr) if arr == vec![1.1, 2.2, 3.3])
    );

    let result = parse_value("true false true", "array[boolean]", "test.path");
    assert!(
        matches!(result.unwrap(), PropertyValue::BooleanArray(arr) if arr == vec![true, false, true])
    );

    let result = parse_value("1 2 3", "array[integer]", "test.path");
    assert!(matches!(result.unwrap(), PropertyValue::IntegerArray(arr) if arr == vec![1, 2, 3]));
}

#[test]
fn test_process_env_vars() {
    let mut property_map = HashMap::new();
    property_map.insert(
        "APP_NAME".to_string(),
        PropertyInfo {
            path: "app.name".to_string(),
            property_type: "string".to_string(),
        },
    );
    property_map.insert(
        "APP_PORT_*".to_string(),
        PropertyInfo {
            path: "app.ports.*".to_string(),
            property_type: "integer".to_string(),
        },
    );

    // Set test environment variables
    unsafe {
        env::set_var("PREFIX_APP_NAME", "test-app");
        env::set_var("PREFIX_APP_PORT_HTTP", "8080");
        env::set_var("PREFIX_APP_PORT_HTTPS", "8443");
    }

    let result = process_env_vars("PREFIX_", &property_map).unwrap();

    assert_eq!(result.len(), 3);

    // Check APP_NAME
    let app_name = result.get("PREFIX_APP_NAME").unwrap();
    assert_eq!(app_name.path, "app.name");
    assert_eq!(app_name.property_type, "string");
    assert!(matches!(&app_name.value, Some(PropertyValue::String(s)) if s == "test-app"));

    // Check APP_PORT_HTTP
    let http_port = result.get("PREFIX_APP_PORT_HTTP").unwrap();
    assert_eq!(http_port.path, "app.ports.http");
    assert_eq!(http_port.property_type, "integer");
    assert!(matches!(&http_port.value, Some(PropertyValue::Integer(i)) if *i == 8080));

    // Check APP_PORT_HTTPS
    let https_port = result.get("PREFIX_APP_PORT_HTTPS").unwrap();
    assert_eq!(https_port.path, "app.ports.https");
    assert_eq!(https_port.property_type, "integer");
    assert!(matches!(&https_port.value, Some(PropertyValue::Integer(i)) if *i == 8443));

    // Clean up environment variables
    unsafe {
        env::remove_var("PREFIX_APP_NAME");
        env::remove_var("PREFIX_APP_PORT_HTTP");
        env::remove_var("PREFIX_APP_PORT_HTTPS");
    }
}
