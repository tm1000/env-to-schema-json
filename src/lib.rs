use serde_json::Value;
use std::collections::HashMap;
use std::env;

/// `PropertyOutput` is a struct representing the output of a processed property.
///
/// - `path`: A string representing the path to the property in the configuration.
/// - `property_type`: A string indicating the type of the property.
/// - `value`: An optional `PropertyValue` that holds the parsed value of the property.
#[derive(Debug, Clone)]
pub struct PropertyOutput {
    pub path: String,
    pub property_type: String,
    pub value: Option<PropertyValue>,
}

/// `PropertyValue` is an enumeration representing possible types of property values.
///
/// - `String`: Represents a property value as a string.
/// - `Number`: Represents a property value as a floating-point number.
/// - `Boolean`: Represents a property value as a boolean.
/// - `Integer`: Represents a property value as an integer.
/// - `StringArray`: Represents a property value as an array of strings.
/// - `NumberArray`: Represents a property value as an array of floating-point numbers.
/// - `BooleanArray`: Represents a property value as an array of booleans.
/// - `IntegerArray`: Represents a property value as an array of integers.
#[derive(Debug, Clone)]
pub enum PropertyValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Integer(i64),
    StringArray(Vec<String>),
    NumberArray(Vec<f64>),
    BooleanArray(Vec<bool>),
    IntegerArray(Vec<i64>),
}

/// Format a `PropertyValue` as a string, suitable for display to the user.
///
/// For single values, the string representation is used directly. For arrays,
/// the elements are joined with commas and enclosed in square brackets.
///
impl std::fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropertyValue::String(s) => write!(f, "{}", s),
            PropertyValue::Number(n) => write!(f, "{}", n),
            PropertyValue::Boolean(b) => write!(f, "{}", b),
            PropertyValue::Integer(i) => write!(f, "{}", i),
            PropertyValue::StringArray(arr) => write!(f, "[{}]", arr.join(", ")),
            PropertyValue::NumberArray(arr) => write!(
                f,
                "[{}]",
                arr.iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            PropertyValue::BooleanArray(arr) => write!(
                f,
                "[{}]",
                arr.iter()
                    .map(|b| b.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            PropertyValue::IntegerArray(arr) => write!(
                f,
                "[{}]",
                arr.iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

/// Represents information about a property, including its path and type.
#[derive(Debug, Clone)]
pub struct PropertyInfo {
    pub path: String,
    pub property_type: String,
}

/// Recursively traverses a JSON schema object and extracts all scalar properties and array of scalar properties.
///
/// # Arguments
///
/// * `schema` - The JSON schema object to traverse.
/// * `obj` - The current object being traversed.
/// * `prefix` - The prefix to add to the property path.
///
/// # Returns
///
/// A vector of `PropertyInfo` objects, each containing the property path and type.
pub fn get_properties(schema: &Value, obj: &Value, prefix: &str) -> Vec<PropertyInfo> {
    let mut properties = Vec::new();

    // Handle regular properties
    if let Some(props) = obj.get("properties").and_then(|p| p.as_object()) {
        for (key, value) in props {
            let full_key = if prefix.is_empty() {
                key.to_string()
            } else {
                format!("{}.{}", prefix, key)
            };

            // If there's a $ref, follow it
            if let Some(ref_path) = value.get("$ref").and_then(|r| r.as_str()) {
                // Extract the definition name from "#/definitions/xyz"
                if let Some(def_name) = ref_path.strip_prefix("#/definitions/") {
                    if let Some(definitions) = schema.get("definitions").and_then(|d| d.as_object())
                    {
                        if let Some(def) = definitions.get(def_name) {
                            properties.extend(get_properties(schema, def, &full_key));
                            continue;
                        }
                    }
                }
            }

            // Check if this property has a scalar type or is an array of scalars
            if let Some(type_val) = value.get("type").and_then(|t| t.as_str()) {
                match type_val {
                    "string" | "number" | "boolean" | "integer" => {
                        properties.push(PropertyInfo {
                            path: full_key.clone(),
                            property_type: type_val.to_string(),
                        });
                    }
                    "array" => {
                        // Check if array items are scalar types
                        if let Some(items) = value.get("items") {
                            if let Some(item_type) = items.get("type").and_then(|t| t.as_str()) {
                                match item_type {
                                    "string" | "number" | "boolean" | "integer" => {
                                        properties.push(PropertyInfo {
                                            path: full_key.clone(),
                                            property_type: format!("array[{}]", item_type),
                                        });
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Recursively get nested properties
            if value.is_object() {
                properties.extend(get_properties(schema, value, &full_key));
            }
        }
    }

    // Handle additionalProperties
    if let Some(additional_props) = obj.get("additionalProperties") {
        if additional_props.is_object() {
            let full_key = if prefix.is_empty() {
                "*".to_string() // Use * to indicate any key
            } else {
                format!("{}.{}", prefix, "*")
            };
            properties.extend(get_properties(schema, additional_props, &full_key));
        }
    }

    properties
}

/// Parses a string value into a `PropertyValue` based on the specified property type.
///
/// # Arguments
///
/// * `value` - The string value to parse.
/// * `property_type` - The type of the property that determines how to parse the value. Supported types are:
///   - "string": Accepts any string that doesn't resemble a number or boolean.
///   - "number": Parses the value as a floating-point number.
///   - "boolean": Parses the value as a boolean.
///   - "integer": Parses the value as an integer.
///   - "array[string]": Splits the value by spaces into an array of strings.
///   - "array[number]": Splits the value by spaces and parses each as a floating-point number.
///   - "array[boolean]": Splits the value by spaces and parses each as a boolean.
///   - "array[integer]": Splits the value by spaces and parses each as an integer.
/// * `path` - The path to the property, used in error messages.
///
/// # Returns
///
/// A `Result` containing the parsed `PropertyValue` on success, or an error message on failure.
///
pub fn parse_value(
    value: &str,
    property_type: &str,
    path: &str,
) -> Result<PropertyValue, Box<dyn std::error::Error>> {
    match property_type {
        "string" => {
            // For string type, don't accept values that look like other types
            if value.parse::<f64>().is_ok() || value.parse::<bool>().is_ok() {
                Err(format!(
                    "Invalid string value '{}' for {}: value looks like a number or boolean",
                    value, path
                )
                .into())
            } else {
                Ok(PropertyValue::String(value.to_string()))
            }
        }
        "number" => value
            .parse::<f64>()
            .map(PropertyValue::Number)
            .map_err(|_| format!("Invalid number value '{}' for {}", value, path).into()),
        "boolean" => value
            .parse::<bool>()
            .map(PropertyValue::Boolean)
            .map_err(|_| format!("Invalid boolean value '{}' for {}", value, path).into()),
        "integer" => value
            .parse::<i64>()
            .map(PropertyValue::Integer)
            .map_err(|_| format!("Invalid integer value '{}' for {}", value, path).into()),
        "array[string]" => Ok(PropertyValue::StringArray(
            value.split(' ').map(|s| s.trim().to_string()).collect(),
        )),
        "array[number]" => {
            let parsed: Result<Vec<f64>, _> =
                value.split(' ').map(|s| s.trim().parse::<f64>()).collect();
            parsed
                .map(PropertyValue::NumberArray)
                .map_err(|_| format!("Invalid number array value '{}' for {}", value, path).into())
        }
        "array[boolean]" => {
            let parsed: Result<Vec<bool>, _> =
                value.split(' ').map(|s| s.trim().parse::<bool>()).collect();
            parsed
                .map(PropertyValue::BooleanArray)
                .map_err(|_| format!("Invalid boolean array value '{}' for {}", value, path).into())
        }
        "array[integer]" => {
            let parsed: Result<Vec<i64>, _> =
                value.split(' ').map(|s| s.trim().parse::<i64>()).collect();
            parsed
                .map(PropertyValue::IntegerArray)
                .map_err(|_| format!("Invalid integer array value '{}' for {}", value, path).into())
        }
        _ => Err(format!("Unknown property type '{}' for {}", property_type, path).into()),
    }
}

/// Process environment variables with a given prefix and match them to a property map.
///
/// This function takes a prefix and a property map, and then iterates over all environment
/// variables with the given prefix. For each environment variable, it checks if there is a
/// matching pattern in the property map. If there is, it replaces the `*` in the property path
/// with the actual value from the environment key and parses the value according to the
/// property type. The result is stored in a `HashMap` with the original environment key as the
/// key and a `PropertyOutput` struct as the value.
///
/// # Arguments
///
/// * `prefix`: The prefix to look for in environment variables.
/// * `property_map`: A map of property names to `PropertyInfo` structs.
///
/// # Returns
///
/// A `HashMap` of environment keys to `PropertyOutput` structs, or an error if a value could
/// not be parsed according to the property type.
pub fn process_env_vars(
    prefix: &str,
    property_map: &HashMap<String, PropertyInfo>,
) -> Result<HashMap<String, PropertyOutput>, Box<dyn std::error::Error>> {
    let mut result = HashMap::new();

    let env_vars: Vec<(String, String)> = env::vars()
        .filter(|(key, _)| key.starts_with(prefix))
        .collect();

    for (key, raw_value) in env_vars {
        let stripped_key = key.strip_prefix(prefix).unwrap_or(&key);
        let key_parts: Vec<&str> = stripped_key.split('_').collect();

        // Find matching pattern
        for (pattern_key, prop_info) in property_map {
            let pattern_parts: Vec<&str> = pattern_key.split('_').collect();

            if pattern_parts.len() == key_parts.len() {
                let mut matches = true;
                for (i, pattern_part) in pattern_parts.iter().enumerate() {
                    if *pattern_part != "*" && *pattern_part != key_parts[i] {
                        matches = false;
                        break;
                    }
                }
                if matches {
                    // Replace * in the property path with the actual value from the environment key
                    let mut final_path = prop_info.path.clone();

                    for (i, pattern_part) in pattern_parts.iter().enumerate() {
                        if *pattern_part == "*" {
                            final_path = final_path.replacen("*", &key_parts[i].to_lowercase(), 1);
                        }
                    }

                    let parsed_value =
                        parse_value(&raw_value, &prop_info.property_type, &final_path)?;
                    result.insert(
                        key.clone(),
                        PropertyOutput {
                            path: final_path.clone(),
                            property_type: prop_info.property_type.clone(),
                            value: Some(parsed_value),
                        },
                    );
                    break;
                }
            }
        }
    }
    Ok(result)
}
