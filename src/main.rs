use clap::Parser;
use env_to_schema_json::process_env_vars;
use jsonschema::JSONSchema;
use jsonschema::error::{TypeKind, ValidationErrorKind};
use jsonschema::primitive_type::PrimitiveType;
use serde_json::Map;
use serde_json::Value;
use std::io::Read;

fn create_nested_json(config: &mut Map<String, Value>, path: &str, value: &str) {
    let parts: Vec<&str> = path.split('.').collect();

    fn set_nested_value(map: &mut Map<String, Value>, parts: &[&str], value: &str) {
        if parts.is_empty() {
            return;
        }

        let (first, rest) = parts.split_at(1);
        let part = first[0];

        if let Ok(idx) = part.parse::<usize>() {
            // This is an array index, we need to handle the previous part
            if let Some(&prev) = parts.first() {
                let entry = map
                    .entry(prev.to_string())
                    .or_insert_with(|| Value::Array(Vec::new()));
                if let Value::Array(arr) = entry {
                    while arr.len() <= idx {
                        if rest.is_empty() {
                            arr.push(Value::String(value.to_string()));
                        } else {
                            arr.push(Value::Object(Map::new()));
                        }
                    }
                    if !rest.is_empty() {
                        if let Value::Object(next_map) = &mut arr[idx] {
                            set_nested_value(next_map, rest, value);
                        }
                    }
                }
            }
        } else if rest.is_empty() {
            // Final value
            map.insert(part.to_string(), Value::String(value.to_string()));
        } else {
            // Non-numeric key with more parts to process
            let next = &rest[0];
            let entry = if next.parse::<usize>().is_ok() {
                // Next part is numeric, create array
                map.entry(part.to_string())
                    .or_insert_with(|| Value::Array(Vec::new()))
            } else {
                // Next part is a key, create object
                map.entry(part.to_string())
                    .or_insert_with(|| Value::Object(Map::new()))
            };

            match entry {
                Value::Array(arr) => {
                    let idx = next.parse::<usize>().unwrap();
                    while arr.len() <= idx {
                        arr.push(Value::Object(Map::new()));
                    }
                    if let Value::Object(next_map) = &mut arr[idx] {
                        set_nested_value(next_map, &rest[1..], value);
                    }
                }
                Value::Object(next_map) => {
                    set_nested_value(next_map, rest, value);
                }
                _ => unreachable!(),
            }
        }
    }

    set_nested_value(config, &parts, value);
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Prefix to filter environment variables
    #[arg(short, long, default_value = "PREFIX_")]
    prefix: String,

    #[arg(short, long)]
    debug: bool,

    #[arg(short, long)]
    schema: String,
}

fn fix_and_validate_json(
    schema: &Value,
    config: Map<String, Value>,
    retried: bool,
) -> Result<Map<String, Value>, String> {
    // Validate the generated JSON against the schema
    let compiled_schema =
        JSONSchema::compile(&schema).map_err(|e| format!("Failed to compile schema: {}", e))?;

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
                                    if let Some(index) = parent_parts[i + 1].parse::<usize>().ok() {
                                        if index < arr.len() {
                                            if let Value::Object(map) = &mut arr[index] {
                                                in_array = true;
                                                return Some(map)
                                            } else {
                                                println!("Failed to get object at index {}", index);
                                                return None
                                            }
                                        } else {
                                            println!("Index {} out of bounds", index);
                                            return None;
                                        }
                                    }
                                    None
                                }
                                _ => {
                                    println!("Failed to get value at path {}", path_parts.join("."));
                                    None
                                },
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
                                                    .split(|c| c == ' ' || c == ',')
                                                    .filter(|s| !s.is_empty())
                                                    .map(|s| Value::String(s.trim().to_string()))
                                                    .collect();
                                                Ok(Value::Array(items))
                                            }
                                            PrimitiveType::Boolean => {
                                                Err("Unsupported type: Boolean".to_string())
                                            }
                                            PrimitiveType::Integer => {
                                                Err("Unsupported type: Integer".to_string())
                                            }
                                            PrimitiveType::Null => {
                                                Err("Unsupported type: Null".to_string())
                                            }
                                            PrimitiveType::Number => {
                                                Err("Unsupported type: Number".to_string())
                                            }
                                            PrimitiveType::Object => {
                                                Err("Unsupported type: Object".to_string())
                                            }
                                            PrimitiveType::String => {
                                                Err("Unsupported type: String".to_string())
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
                            _ => {
                                return Err(format!("Unsupported type: {:?}", error.kind))
                            }
                        }
                    }
                }
            }
            Ok(fix_and_validate_json(&schema, fixed_config, true)?)
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut schema_content = String::new();

    if args.schema.is_empty() {
        // Read and parse the schema from stdin
        std::io::stdin().read_to_string(&mut schema_content)?;
    } else {
        // Read and parse the schema from file
        schema_content = std::fs::read_to_string(args.schema)?;
    }
    let schema: Value = serde_json::from_str(&schema_content)?;

    let result = process_env_vars(&args.prefix)?;

    let mut config = Map::new();

    for (_env_var, props) in result {
        create_nested_json(&mut config, &props.path, &props.value);
    }

    let validated_config = fix_and_validate_json(&schema, config.clone(), false)?;
    let config_json = serde_json::to_string_pretty(&Value::Object(validated_config))?;
    println!("{}", config_json);

    Ok(())
}
