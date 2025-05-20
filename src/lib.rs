use jsonschema::JSONSchema;
use jsonschema::error::{TypeKind, ValidationErrorKind};
use jsonschema::primitive_type::PrimitiveType;
use serde_json::Map;
use serde_json::Value;
use std::{collections::HashMap, env};

#[derive(Debug, Clone)]
pub struct EnvProperty {
    pub env: String,
    pub value: String,
    pub path: String,
}

/// Fix and validate the generated JSON against the schema. This function
/// takes the input JSON and the schema as a JSON object, and returns a
/// Result containing the validated JSON. If the JSON is invalid, a String
/// containing the error messages is returned. If the JSON is valid, the
/// same JSON is returned.
///
/// If the JSON is invalid, the function will try to fix the errors by
/// converting the values to the correct type. This is done by parsing the
/// error messages and modifying the JSON accordingly. If the errors cannot
/// be fixed, the function will return an error message.
///
/// The function takes an additional parameter `retried` which indicates
/// whether the function has been called before. If `retried` is false, the
/// function will try to fix the errors and call itself recursively. If
/// `retried` is true, the function will return an error message without
/// trying to fix the errors.
pub fn fix_and_validate_json(
    schema: &Value,
    config: Map<String, Value>,
    retried: bool,
) -> Result<Map<String, Value>, String> {
    // Validate the generated JSON against the schema
    let compiled_schema =
        JSONSchema::compile(schema).map_err(|e| format!("Failed to compile schema: {}", e))?;

    let instance = Value::Object(config.clone());

    match compiled_schema.validate(&instance) {
        Ok(_) => Ok(config),
        Err(errors) => {
            if retried {
                // Convert validation errors to a string
                let error_messages: Vec<String> = errors.map(|e| e.to_string()).collect();
                return Err(error_messages.join(", "));
            }

            let mut fixed_config = config.clone();
            for error in errors {
                // Collect all path chunks to build the full path
                let mut path_parts: Vec<String> = Vec::new();
                for path in error.instance_path.iter() {
                    if let jsonschema::paths::PathChunk::Property(prop) = path {
                        path_parts.push(prop.as_ref().to_string());
                        continue;
                    }
                    if let jsonschema::paths::PathChunk::Index(idx) = path {
                        path_parts.push(idx.to_string());
                        continue;
                    }
                }

                if let Some((last_part, parent_parts)) = path_parts.split_last() {
                    let mut current = &mut fixed_config;
                    let mut in_array = false;
                    for (i, part) in parent_parts.iter().enumerate() {
                        if in_array {
                            in_array = false;
                            continue;
                        }

                        current = current
                            .get_mut(part)
                            .and_then(|v| match v {
                                Value::Object(map) => Some(map),
                                Value::Array(arr) => {
                                    if let Ok(index) = parent_parts[i + 1].parse::<usize>() {
                                        if index < arr.len() {
                                            if let Value::Object(map) = &mut arr[index] {
                                                in_array = true;
                                                return Some(map);
                                            } else {
                                                println!("Failed to get object at index {}", index);
                                                return None;
                                            }
                                        } else {
                                            println!("Index {} out of bounds", index);
                                            return None;
                                        }
                                    }
                                    None
                                }
                                _ => {
                                    println!(
                                        "Failed to get value at path {}",
                                        path_parts.join(".")
                                    );
                                    None
                                }
                            })
                            .unwrap();
                    }

                    let existing = current.get(last_part.as_str()).cloned().unwrap();

                    if let ValidationErrorKind::Type { kind } = &error.kind {
                        match kind {
                            TypeKind::Single(primitive_type) => {
                                let new_value: Result<Value, String> = match existing {
                                    Value::String(existing) => {
                                        match primitive_type {
                                            PrimitiveType::Array => {
                                                // Split by spaces or commas and trim each item
                                                let items: Vec<Value> = existing
                                                    .split([' ', ','])
                                                    .filter(|s| !s.is_empty())
                                                    .map(|s| Value::String(s.trim().to_string()))
                                                    .collect();
                                                Ok(Value::Array(items))
                                            }
                                            PrimitiveType::Boolean => {
                                                if let Ok(value) = existing.parse::<bool>() {
                                                    Ok(Value::Bool(value))
                                                } else {
                                                    Err("Unsupported type: Boolean".to_string())
                                                }
                                            }
                                            PrimitiveType::Integer => {
                                                if let Ok(value) = existing.parse::<i64>() {
                                                    Ok(Value::Number(value.into()))
                                                } else {
                                                    Err("Unsupported type: Integer".to_string())
                                                }
                                            }
                                            PrimitiveType::Null => {
                                                Err("Unsupported type: Null".to_string())
                                            }
                                            PrimitiveType::Number => {
                                                if let Ok(value) =
                                                    existing.parse::<serde_json::Number>()
                                                {
                                                    Ok(Value::Number(value))
                                                } else {
                                                    Err("Unsupported type: Number".to_string())
                                                }
                                            }
                                            PrimitiveType::Object => {
                                                Err("Unsupported type: Object".to_string())
                                            }
                                            PrimitiveType::String => {
                                                Ok(Value::String(existing.clone()))
                                            }
                                        }
                                    }
                                    _ => Err(format!(
                                        "Existing value is not a string: {:#?}",
                                        existing
                                    )),
                                };
                                current.insert(last_part.to_string(), new_value.unwrap());
                            }
                            _ => return Err(format!("Unsupported type: {:?}", error.kind)),
                        }
                    }
                }
            }
            Ok(fix_and_validate_json(schema, fixed_config, true)?)
        }
    }
}

/// Recursively creates a nested JSON object based on the given `path` and sets the value
/// to the given `value`.
///
/// The `path` is split by dots (`.`) and each part is used to create a nested JSON
/// object. If the part is a number, it is used as an array index, otherwise it is used as
/// a key in an object.
///
/// For example, if the `path` is `"a.b.0.c"`, the JSON object will look like this:
///
///
pub fn create_nested_json(config: &mut Map<String, Value>, path: &str, value: &str) {
    let parts: Vec<&str> = path.split('.').collect();

    fn set_nested_value(map: &mut Map<String, Value>, parts: &[&str], value: &str) {
        if parts.is_empty() {
            return;
        }

        let (first, rest) = parts.split_at(1);
        let part = first[0];

        if rest.is_empty() {
            // Final value
            map.insert(part.to_string(), Value::String(value.to_string()));
            return;
        }

        let next = &rest[0];
        let is_next_array_index = next.parse::<usize>().is_ok();

        let entry = map.entry(part.to_string()).or_insert_with(|| {
            if is_next_array_index {
                Value::Array(Vec::new())
            } else {
                Value::Object(Map::new())
            }
        });

        match entry {
            Value::Array(arr) => {
                let idx = next.parse::<usize>().unwrap();
                while arr.len() <= idx {
                    if rest.len() == 1 {
                        // If this is the last part, use the value directly
                        arr.push(Value::String(value.to_string()));
                    } else {
                        arr.push(Value::Object(Map::new()));
                    }
                }
                if rest.len() > 1 {
                    if let Value::Object(next_map) = &mut arr[idx] {
                        set_nested_value(next_map, &rest[1..], value);
                    }
                }
            }
            Value::Object(next_map) => {
                set_nested_value(next_map, rest, value);
            }
            _ => unreachable!(),
        }
    }

    set_nested_value(config, &parts, value);
}

/// Processes environment variables that start with a given prefix and
/// returns a `HashMap` where each key is the original environment variable
/// name, and each value is an `EnvProperty` containing:
/// - `env`: the original environment variable name,
/// - `value`: the value of the environment variable,
/// - `path`: a transformed version of the key where double underscores (`__`)
///   are replaced with underscores, underscores (`_`) are replaced with dots (`.`),
///   and the whole path is converted to lowercase.
///
/// # Arguments
///
/// * `prefix` - A string slice that holds the prefix to filter environment variables.
///
/// # Returns
///
/// * `Result<HashMap<String, EnvProperty>, Box<dyn std::error::Error>>` - A result containing
///   a `HashMap` of environment variables matching the prefix transformed into `EnvProperty`
///   structs, or an error.
pub fn process_env_vars(
    prefix: &str,
) -> Result<HashMap<String, EnvProperty>, Box<dyn std::error::Error>> {
    let mut result = HashMap::new();

    let env_vars: Vec<(String, String)> = env::vars()
        .filter(|(key, _)| key.starts_with(prefix))
        .collect();

    for (key, raw_value) in env_vars {
        let stripped_key = key.strip_prefix(prefix).unwrap_or(&key);
        let path = stripped_key
            .replace("__", "||||")
            .split('_')
            .collect::<Vec<&str>>()
            .join(".")
            .to_lowercase()
            .replace("||||", "_");

        // Remove quotes from the start and end of the value if present
        let trimmed_value = raw_value.trim();
        let value = match (trimmed_value.starts_with('"') && trimmed_value.ends_with('"'))
            || (trimmed_value.starts_with('\'') && trimmed_value.ends_with('\''))
        {
            true => {
                let len = trimmed_value.len();
                if len >= 2 {
                    trimmed_value[1..len - 1].to_string()
                } else {
                    trimmed_value.to_string()
                }
            }
            false => raw_value.clone(),
        };

        result.insert(
            key.clone(),
            EnvProperty {
                env: key.clone(),
                value,
                path,
            },
        );
    }
    Ok(result)
}

/// Resolves a reference path within a JSON schema to retrieve the associated value.
///
/// This function takes a JSON schema and a reference path (in the form of a string),
/// and traverses the schema to locate the value specified by the reference path. The
/// reference path should be formatted as a JSON Pointer, with components separated by
/// slashes (`/`). If the reference path starts with a `#/`, this prefix will be removed
/// before processing.
///
/// # Arguments
///
/// * `schema` - A reference to a JSON `Value` representing the schema to be traversed.
/// * `ref_path` - A string slice specifying the reference path to resolve.
///
/// # Returns
///
/// * `Option<&'a Value>` - Returns an `Option` containing a reference to the value
///   pointed to by the reference path, or `None` if any component of the path is not
///   found within the schema.
pub fn resolve_ref<'a>(schema: &'a Value, ref_path: &str) -> Option<&'a Value> {
    // Remove the '#/' prefix if present
    let clean_path = ref_path.trim_start_matches("#/");

    // Split the path into components
    let components: Vec<&str> = clean_path.split('/').collect();

    // Start from the root and traverse
    let mut current = schema;
    for component in components {
        current = current.get(component)?;
    }

    Some(current)
}
