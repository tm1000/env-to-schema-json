# env-to-schema-json

A Rust CLI tool that converts environment variables into a JSON object validated against a [JSON Schema](https://json-schema.org/).

## Overview

Define your configuration structure using JSON Schema, set environment variables with a prefix, and get a valid JSON configuration — no manual parsing code required.

The tool transforms prefixed environment variables into a nested JSON object, then validates and auto-fixes type mismatches against your schema.

## Features

- **Schema-driven validation** — validates generated JSON against any valid JSON Schema
- **Automatic type fixing** — converts string values to match schema types (integers, booleans, arrays, etc.)
- **Nested object support** — use `__` to create nested structures (e.g., `DB__HOST` → `db.host`)
- **Array support** — numeric path segments create array elements
- **Stdin or file input** — pipe a schema or provide a file path

## Example

Given a JSON Schema:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "first_name": { "type": "string" },
    "last_name": { "type": "string" },
    "age": { "type": "integer", "minimum": 0 }
  }
}
```

With these environment variables:

```bash
export PERSON_FIRST_NAME=John
export PERSON_LAST_NAME=Doe
export PERSON_AGE=30
```

Run:

```bash
cat schema.json | env-to-schema-json --prefix PERSON_
```

Output:

```json
{
  "first_name": "John",
  "last_name": "Doe",
  "age": 30
}
```

## Installation

### From source

```bash
cargo install env-to-schema-json
```

### Cargo binary

```bash
cargo build --release
cp target/release/env-to-schema-json /usr/local/bin/
```

## Usage

```bash
env-to-schema-json --prefix <PREFIX> [schema.json]
```

| Flag | Default | Description |
|------|---------|-------------|
| `-p, --prefix` | `PREFIX_` | Prefix to filter environment variables |
| `-s, --schema` | _(none)_ | Path to JSON schema file (omit to read from stdin) |
| `-d, --debug` | `false` | Print the generated JSON before validation |

### Reading from stdin

```bash
cat schema.json | env-to-schema-json --prefix MYAPP_
```

### Reading from a file

```bash
env-to-schema-json --prefix MYAPP_ schema.json
```

## Path Transformation

Environment variable names are transformed into JSON paths using these rules:

| Env Var | JSON Path |
|---------|-----------|
| `APP_HOST=localhost` | `app.host` |
| `APP_DB__HOST=localhost` | `app.db_host` |
| `APP_SERVERS__0_HOST=a` | `app.servers[0].host` |
| `APP_SERVERS__1_HOST=b` | `app.servers[1].host` |

- Underscores (`_`) become dots (`.`) — creating nested objects
- Double underscores (`__`) become literal underscores (`_`) — for flat keys containing underscores
- Numeric path segments create array elements
- All keys are converted to lowercase

## Development

```bash
# Run
cargo run -- --prefix PREFIX_

# Run tests
cargo test

# Run with schema file
cargo run -- --prefix CADDY_ --schema example/caddy-schema.json
```
