use serde_json::Value;
use std::{collections::HashMap, env};

#[derive(Debug, Clone)]
pub struct EnvProperty {
    pub env: String,
    pub value: String,
    pub path: String,
}

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

        result.insert(
            key.clone(),
            EnvProperty {
                env: key.clone(),
                value: raw_value.clone(),
                path: path,
            },
        );
    }
    Ok(result)
}

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
