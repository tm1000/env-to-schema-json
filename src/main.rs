use clap::Parser;
use env_to_schema_json::{create_nested_json, fix_and_validate_json, process_env_vars};
use serde_json::Map;
use serde_json::Value;
use std::io::Read;

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

/// Main function that processes environment variables and validates them against a JSON schema.
///
/// This function takes a prefix to filter environment variables, a boolean flag to enable
/// debug mode, and a string path to a JSON schema file. It processes the environment variables
/// that start with the given prefix, creates a nested JSON object based on the transformed
/// environment variable names, and validates the JSON object against the schema. If the JSON
/// object is invalid, it attempts to fix the errors by converting the values to the correct type.
/// If the JSON object is valid, it prints the validated JSON object to the console.
///
/// # Arguments
///
/// * `args` - A struct containing the command-line arguments.
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - A result containing either an empty tuple or an error.
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
