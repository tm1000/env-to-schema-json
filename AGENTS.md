# AGENTS.md

## Project Overview

`env-to-schema-json` is a Rust CLI tool that converts environment variables into a JSON object validated against a [JSON Schema](https://json-schema.org/). It transforms prefixed environment variables into a nested JSON structure, then validates and auto-fixes type mismatches against the provided schema.

**Key features:**

- Schema-driven validation using JSON Schema
- Automatic type fixing (strings to integers, booleans, arrays, etc.)
- Nested object support via `__` separator (e.g., `DB__HOST` → `db.host`)
- Array support via numeric path segments
- Stdin or file input for schema

**Tech stack:** Rust 2024 edition, `clap` (CLI), `serde`/`serde_json` (serialization), `jsonschema` (validation).

## Setup Commands

```bash
# Install Rust (if not installed)
# Use rustup: https://rustup.rs/

# Clone and enter the project
cd env-to-schema-json

# Build debug
cargo build

# Build release
cargo build --release

# Install globally
cargo install --path .
```

## Development Workflow

```bash
# Run the CLI (debug mode)
cargo run -- --prefix PREFIX_

# Run with a schema file
cargo run -- --prefix CADDY_ --schema example/caddy-schema.json

# Pipe schema from stdin
cat example/basic-schema.json | cargo run -- --prefix MYAPP_

# Enable debug output (prints generated JSON before validation)
cargo run -- --prefix PREFIX_ --schema example/basic-schema.json --debug
```

## Testing Instructions

```bash
# Run all tests
cargo test

# Run library tests only
cargo test --lib

# Run integration tests (main binary) only
cargo test --test main_tests

# Run a specific test
cargo test test_process_env_vars

# Run tests with output
cargo test -- --nocapture
```

**Test file locations:**

- `tests/lib_tests.rs` — Unit tests for library functions (`process_env_vars`, `create_nested_json`, `fix_and_validate_json`, `resolve_ref`)
- `tests/main_tests.rs` — Integration tests for the CLI binary (schema file input, stdin input, debug flag, error handling)

**Test conventions:**

- Tests that modify environment variables use `unsafe { env::set_var(...) }` and clean up with `env::remove_var(...)`
- Integration tests use `tempfile::NamedTempFile` for schema files and `std::process::Command` to spawn the binary
- Tests cover: path transformation, quote stripping, nested objects, arrays, type fixing, error cases, and edge cases

## Code Style

```bash
# Format code
cargo fmt

# Check formatting (used in CI)
cargo fmt -- --check

# Lint with clippy
cargo clippy

# Fail on clippy warnings (used in CI)
cargo clippy -- -D warnings
```

**Conventions:**

- All commands must pass (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`) before committing
- Public functions have doc comments with `# Arguments` and `# Returns` sections
- Error handling uses `Result<T, Box<dyn std::error::Error>>` for CLI and `Result<T, String>` for library functions
- Environment variable path transformation: `_` → `.`, `__` → `_`, all keys lowercased

## Build and Deployment

```bash
# Build for current platform
cargo build --release

# Cross-compile for Linux
cargo build --release --target x86_64-unknown-linux-gnu

# Cross-compile for macOS
cargo build --release --target x86_64-apple-darwin

# Cross-compile for Windows
cargo build --release --target x86_64-pc-windows-msvc

# Install from source
cargo install env-to-schema-json
```

**CI/CD (GitHub Actions):**

- `.github/workflows/tests.yml` — Runs on push/PR to `main`; tests on ubuntu, macos, windows; runs clippy and fmt checks
- `.github/workflows/release.yml` — Triggered on `v*` tags; builds release binaries and publishes GitHub Releases

## CLI Usage

```bash
env-to-schema-json --prefix <PREFIX> [schema.json]
```

| Flag           | Default   | Description                                        |
| -------------- | --------- | -------------------------------------------------- |
| `-p, --prefix` | `PREFIX_` | Prefix to filter environment variables             |
| `-s, --schema` | _(none)_  | Path to JSON schema file (omit to read from stdin) |
| `-d, --debug`  | `false`   | Print the generated JSON before validation         |

## Path Transformation Rules

| Env Var                  | JSON Path             |
| ------------------------ | --------------------- |
| `APP_HOST=localhost`     | `app.host`            |
| `APP_DB__HOST=localhost` | `app.db_host`         |
| `APP_SERVERS__0_HOST=a`  | `app.servers[0].host` |
| `APP_SERVERS__1_HOST=b`  | `app.servers[1].host` |

- `_` → `.` (nested objects)
- `__` → `_` (literal underscore in key)
- Numeric segments → array indices
- All keys → lowercase
- All other characters pass through unchanged

**Dashes:** the transformation never _produces_ a `-`, but it _preserves_ one.
A key needing a dash (`X-Forwarded-For`) must spell it literally; `__` escapes
to an underscore, so `X__FORWARDED__FOR` yields the unrelated key
`x_forwarded_for`. Such a name can't be `export`ed from a shell — set it via a
container runtime or `env 'NAME=value'`.

**Array indices** must start at 0 and be contiguous. Gaps leave unset elements
that fail validation, reported with the names of the variables involved.

## Example Files

- `example/basic-schema.json` — Basic schema with string, integer, boolean, array types
- `example/caddy-schema.json` — Real-world Caddy web server configuration schema
- `example/caddy-config.json` — Example Caddy environment variable configuration

## Additional Notes

- The `fix_and_validate_json` function repeatedly attempts to fix type mismatches (e.g., `"5432"` → `5432` for integers), re-validating after each pass until the instance is valid or stops changing (bounded by `MAX_FIX_PASSES`). Passing `retried: true` validates once and reports without fixing anything.
- Values are located with JSON pointers taken straight from the validator, so array elements are handled like object properties.
- Array values are split by spaces or commas when converting to `type: "array"`. The split yields strings; a later pass coerces them against the `items` schema, so `"80 443"` against `items: {type: integer}` becomes `[80, 443]`.
- `fix_and_validate_json_with_sources` additionally takes a map of JSON pointer → environment variable name, so validation failures name the variable responsible. `main.rs` builds this via `path_to_pointer`.
- Quoted values (single or double) are unquoted during processing, unless the quotes are unterminated.
- The `resolve_ref` helper function traverses JSON schema references (e.g., `#/definitions/address`).
