use clap::Parser;
use env_to_schema_json::{PropertyInfo, PropertyValue, get_properties, process_env_vars};
use jsonschema::{Draft, JSONSchema};
use serde_json::Value;
use std::{collections::HashMap, io::Read};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Prefix to filter environment variables
    #[arg(short, long, default_value = "PREFIX_")]
    prefix: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Read and parse the schema from stdin
    let mut schema_content = String::new();
    std::io::stdin().read_to_string(&mut schema_content)?;
    let schema: Value = serde_json::from_str(&schema_content)?;

    // Get all properties recursively
    let properties = get_properties(&schema, &schema, "");

    // Create a HashMap with pattern keys and property info
    let mut property_map: HashMap<String, PropertyInfo> = HashMap::new();
    for prop in &properties {
        let pattern_key = prop
            .path
            .replace('.', "_")
            .replace("*", "STAR")
            .to_uppercase()
            .replace("STAR", "*");
        property_map.insert(pattern_key, prop.clone());
    }

    let result = process_env_vars(&args.prefix, &property_map)?;

    // Create JSON output
    let mut json_output = serde_json::Map::new();
    for output in result.values() {
        if let Some(value) = &output.value {
            let path_parts: Vec<&str> = output.path.split('.').collect();
            let mut current = &mut json_output;

            // Create nested objects for each path part except the last
            for part in path_parts[..path_parts.len() - 1].iter() {
                current = current
                    .entry(*part)
                    .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
                    .as_object_mut()
                    .unwrap();
            }

            // Insert the value at the last path part
            let last_part = path_parts.last().unwrap();
            let json_value = match value {
                PropertyValue::String(s) => serde_json::Value::String(s.clone()),
                PropertyValue::Number(n) => {
                    serde_json::Value::Number(serde_json::Number::from_f64(*n).unwrap())
                }
                PropertyValue::Boolean(b) => serde_json::Value::Bool(*b),
                PropertyValue::Integer(i) => serde_json::Value::Number((*i).into()),
                PropertyValue::StringArray(arr) => serde_json::Value::Array(
                    arr.iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect(),
                ),
                PropertyValue::NumberArray(arr) => serde_json::Value::Array(
                    arr.iter()
                        .map(|n| {
                            serde_json::Value::Number(serde_json::Number::from_f64(*n).unwrap())
                        })
                        .collect(),
                ),
                PropertyValue::BooleanArray(arr) => serde_json::Value::Array(
                    arr.iter().map(|b| serde_json::Value::Bool(*b)).collect(),
                ),
                PropertyValue::IntegerArray(arr) => serde_json::Value::Array(
                    arr.iter()
                        .map(|i| serde_json::Value::Number((*i).into()))
                        .collect(),
                ),
            };

            // Validate that the value matches the expected type
            match output.property_type.as_str() {
                "string" => {
                    if !json_value.is_string() {
                        return Err(
                            format!("Invalid type for {}: expected string", output.path).into()
                        );
                    }
                }
                "number" => {
                    if !json_value.is_number() {
                        return Err(
                            format!("Invalid type for {}: expected number", output.path).into()
                        );
                    }
                }
                "boolean" => {
                    if !json_value.is_boolean() {
                        return Err(
                            format!("Invalid type for {}: expected boolean", output.path).into(),
                        );
                    }
                }
                "integer" => {
                    if !json_value.is_number() {
                        return Err(
                            format!("Invalid type for {}: expected integer", output.path).into(),
                        );
                    }
                }
                t if t.starts_with("array[") => {
                    if !json_value.is_array() {
                        return Err(
                            format!("Invalid type for {}: expected array", output.path).into()
                        );
                    }
                }
                _ => {}
            }

            current.insert(last_part.to_string(), json_value);
        }
    }

    let json_value = serde_json::Value::Object(json_output);
    println!("{}", serde_json::to_string_pretty(&json_value)?);

    // Validate against schema
    let schema = JSONSchema::options()
        .with_draft(Draft::Draft7)
        .compile(&schema)
        .map_err(|e| format!("Failed to compile schema: {}", e))?;

    schema
        .validate(&json_value)
        .map_err(|mut errors| format!("Invalid configuration: {}", errors.next().unwrap()))?;

    Ok(())
}
