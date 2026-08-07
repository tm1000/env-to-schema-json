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

/// Maximum number of coercion passes attempted before giving up. Each pass can
/// only make a value more specific (string -> array -> typed array items), so a
/// small bound is enough to reach a fixed point on any real schema.
const MAX_FIX_PASSES: usize = 8;

/// A validation error captured as owned data, so the instance can be mutated
/// after the borrow taken by the validator has been released.
struct Pending {
    pointer: String,
    target: Option<PrimitiveType>,
    message: String,
}

/// Converts an internal dotted path (`servers.0.port`) into an RFC 6901 JSON
/// pointer (`/servers/0/port`), escaping `~` and `/` so the result can be
/// compared against the pointers reported by the validator.
///
/// # Arguments
///
/// * `path` - The dotted path produced by [`process_env_vars`].
///
/// # Returns
///
/// * `String` - The equivalent JSON pointer.
pub fn path_to_pointer(path: &str) -> String {
    let mut pointer = String::new();
    for segment in path.split('.') {
        pointer.push('/');
        pointer.push_str(&segment.replace('~', "~0").replace('/', "~1"));
    }
    pointer
}

/// Collects the current validation errors as owned [`Pending`] entries. An
/// empty result means the instance validates cleanly.
fn collect_pending(compiled_schema: &JSONSchema, instance: &Value) -> Vec<Pending> {
    match compiled_schema.validate(instance) {
        Ok(()) => Vec::new(),
        Err(errors) => errors
            .map(|error| Pending {
                pointer: error.instance_path.to_string(),
                target: match &error.kind {
                    ValidationErrorKind::Type {
                        kind: TypeKind::Single(primitive_type),
                    } => Some(*primitive_type),
                    _ => None,
                },
                message: error.to_string(),
            })
            .collect(),
    }
}

/// Converts a string into the requested primitive type.
fn coerce_string(existing: &str, target: PrimitiveType) -> Result<Value, String> {
    match target {
        PrimitiveType::Array => {
            // Split by spaces or commas and trim each item. Items stay strings
            // here; a later pass coerces them against the `items` schema.
            let items: Vec<Value> = existing
                .split([' ', ','])
                .filter(|s| !s.is_empty())
                .map(|s| Value::String(s.trim().to_string()))
                .collect();
            Ok(Value::Array(items))
        }
        PrimitiveType::Boolean => existing
            .parse::<bool>()
            .map(Value::Bool)
            .map_err(|_| "Unsupported type: Boolean".to_string()),
        PrimitiveType::Integer => existing
            .parse::<i64>()
            .map(|value| Value::Number(value.into()))
            .map_err(|_| "Unsupported type: Integer".to_string()),
        PrimitiveType::Number => existing
            .parse::<serde_json::Number>()
            .map(Value::Number)
            .map_err(|_| "Unsupported type: Number".to_string()),
        PrimitiveType::String => Ok(Value::String(existing.to_string())),
        PrimitiveType::Null => Err("Unsupported type: Null".to_string()),
        PrimitiveType::Object => Err("Unsupported type: Object".to_string()),
    }
}

/// Attempts to coerce the value at a single error location.
///
/// Returns `Ok(true)` if the instance changed, `Ok(false)` if there was nothing
/// to do (the error is not a single-type mismatch, the location no longer
/// exists, or the value is not a string), and `Err` if the value cannot be
/// represented as the required type.
fn coerce_at(instance: &mut Value, item: &Pending) -> Result<bool, String> {
    let Some(target) = item.target else {
        return Ok(false);
    };
    let Some(slot) = instance.pointer_mut(&item.pointer) else {
        return Ok(false);
    };
    let Value::String(existing) = slot else {
        return Ok(false);
    };

    let replacement = coerce_string(&existing.clone(), target)?;
    if replacement == *slot {
        return Ok(false);
    }
    *slot = replacement;
    Ok(true)
}

/// Renders one validation failure, naming the environment variable responsible
/// where one is known.
fn describe(
    pointer: &str,
    message: &str,
    instance: &Value,
    sources: &HashMap<String, String>,
) -> String {
    if pointer.is_empty() {
        return message.to_string();
    }

    if let Some(env_var) = sources.get(pointer) {
        return format!("{} (from {}): {}", pointer, env_var, message);
    }

    // A null at a location no environment variable set is padding inserted by
    // `create_nested_json` to fill a gap in the array indices.
    if matches!(instance.pointer(pointer), Some(Value::Null))
        && let Some((parent, _)) = pointer.rsplit_once('/')
    {
        let prefix = format!("{}/", parent);
        let mut siblings: Vec<&str> = sources
            .iter()
            .filter(|(candidate, _)| candidate.starts_with(&prefix))
            .map(|(_, env_var)| env_var.as_str())
            .collect();
        if !siblings.is_empty() {
            siblings.sort_unstable();
            return format!(
                "{}: no environment variable sets this array element. Array indices must start at 0 and be contiguous. Variables setting this array: {}",
                pointer,
                siblings.join(", ")
            );
        }
    }

    format!("{}: {}", pointer, message)
}

/// Renders every outstanding validation failure as a single message.
fn describe_all(
    pending: &[Pending],
    instance: &Value,
    sources: &HashMap<String, String>,
) -> String {
    pending
        .iter()
        .map(|item| describe(&item.pointer, &item.message, instance, sources))
        .collect::<Vec<String>>()
        .join(", ")
}

/// Fix and validate the generated JSON against the schema.
///
/// See [`fix_and_validate_json_with_sources`]; this variant reports failures by
/// JSON pointer only, without naming the originating environment variables.
///
/// # Arguments
///
/// * `schema` - The JSON schema to validate against.
/// * `config` - The generated configuration object.
/// * `retried` - When true, validate and report without attempting any fixes.
///
/// # Returns
///
/// * `Result<Map<String, Value>, String>` - The validated configuration, or a
///   description of every failure that could not be fixed.
pub fn fix_and_validate_json(
    schema: &Value,
    config: Map<String, Value>,
    retried: bool,
) -> Result<Map<String, Value>, String> {
    fix_and_validate_json_with_sources(schema, config, retried, &HashMap::new())
}

/// Fix and validate the generated JSON against the schema, repeatedly coercing
/// mismatched values until the instance validates or stops changing.
///
/// Each pass asks the validator what is wrong, coerces every string whose type
/// does not match, and validates again. Iterating this way is what allows a
/// value to be refined more than once — `"80 443"` becomes `["80", "443"]` on
/// one pass and `[80, 443]` on the next, against `items: {type: integer}`.
///
/// Locations are addressed with JSON pointers taken straight from the
/// validator, so array elements are handled the same as object properties.
///
/// # Arguments
///
/// * `schema` - The JSON schema to validate against.
/// * `config` - The generated configuration object.
/// * `retried` - When true, validate and report without attempting any fixes.
/// * `sources` - Maps a JSON pointer to the environment variable that set it,
///   used to name the offending variable in error messages.
///
/// # Returns
///
/// * `Result<Map<String, Value>, String>` - The validated configuration, or a
///   description of every failure that could not be fixed.
pub fn fix_and_validate_json_with_sources(
    schema: &Value,
    config: Map<String, Value>,
    retried: bool,
    sources: &HashMap<String, String>,
) -> Result<Map<String, Value>, String> {
    let compiled_schema =
        JSONSchema::compile(schema).map_err(|e| format!("Failed to compile schema: {}", e))?;

    let mut instance = Value::Object(config);
    let passes = if retried { 1 } else { MAX_FIX_PASSES };

    for _ in 0..passes {
        let pending = collect_pending(&compiled_schema, &instance);
        if pending.is_empty() {
            return Ok(into_object(instance));
        }
        if retried {
            return Err(describe_all(&pending, &instance, sources));
        }

        let mut progressed = false;
        for item in &pending {
            match coerce_at(&mut instance, item) {
                Ok(true) => progressed = true,
                Ok(false) => {}
                Err(reason) => {
                    return Err(describe(&item.pointer, &reason, &instance, sources));
                }
            }
        }

        // Nothing left that we know how to change: report what is still wrong
        // rather than spinning through the remaining passes.
        if !progressed {
            return Err(describe_all(&pending, &instance, sources));
        }
    }

    let pending = collect_pending(&compiled_schema, &instance);
    if pending.is_empty() {
        Ok(into_object(instance))
    } else {
        Err(describe_all(&pending, &instance, sources))
    }
}

/// Unwraps the root object. The instance is built from a `Map` and only ever
/// mutated below the root, so the object variant always holds.
fn into_object(instance: Value) -> Map<String, Value> {
    match instance {
        Value::Object(map) => map,
        _ => Map::new(),
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
                // Pad with placeholders so array length is independent of
                // processing order; the actual value for `idx` is always
                // assigned explicitly below.
                while arr.len() <= idx {
                    arr.push(Value::Null);
                }
                if rest.len() > 1 {
                    if !matches!(arr[idx], Value::Object(_)) {
                        arr[idx] = Value::Object(Map::new());
                    }
                    if let Value::Object(next_map) = &mut arr[idx] {
                        set_nested_value(next_map, &rest[1..], value);
                    }
                } else {
                    arr[idx] = Value::String(value.to_string());
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
/// Every other character is carried through untouched, so a key that needs a
/// character the transformation never produces — a dash, most commonly, as in
/// the HTTP header `X-Forwarded-For` — must contain it literally. Note that
/// such a name cannot be `export`ed from a POSIX shell; set it through a
/// container runtime, or with `env 'NAME=value'`.
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
