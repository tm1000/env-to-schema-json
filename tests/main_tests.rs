use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

#[test]
fn test_main_with_schema_file() {
    unsafe {
        // Create a temporary schema file
        let mut schema_file = NamedTempFile::new().unwrap();
        schema_file
            .write_all(
                br#"{
            "type": "object",
            "properties": {
                "database": {
                    "type": "object",
                    "properties": {
                        "port": {"type": "number"},
                        "enabled": {"type": "boolean"}
                    }
                }
            }
        }"#,
            )
            .unwrap();
        schema_file.flush().unwrap();

        // Set test environment variables
        std::env::set_var("PREFIX_DATABASE_PORT", "5432");
        std::env::set_var("PREFIX_DATABASE_ENABLED", "true");

        // Run the main program
        let output = Command::new(env!("CARGO_BIN_EXE_env-to-schema-json"))
            .arg("--prefix")
            .arg("PREFIX_")
            .arg("--schema")
            .arg(schema_file.path())
            .output()
            .unwrap();

        // Clean up
        std::env::remove_var("PREFIX_DATABASE_PORT");
        std::env::remove_var("PREFIX_DATABASE_ENABLED");

        // Check output
        let stdout = String::from_utf8(output.stdout).unwrap();
        let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

        assert_eq!(json["database"]["port"], 5432);
        assert_eq!(json["database"]["enabled"], true);
        assert!(output.status.success());
    }
}

#[test]
fn test_main_with_schema_from_stdin() {
    unsafe {
        std::env::set_var("STDINPREFIX_COUNT", "7");

        let mut child = Command::new(env!("CARGO_BIN_EXE_env-to-schema-json"))
            .arg("--prefix")
            .arg("STDINPREFIX_")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        child
            .stdin
            .take()
            .unwrap()
            .write_all(br#"{"type":"object","properties":{"count":{"type":"number"}}}"#)
            .unwrap();

        let output = child.wait_with_output().unwrap();

        std::env::remove_var("STDINPREFIX_COUNT");

        let stdout = String::from_utf8(output.stdout).unwrap();
        let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

        assert_eq!(json["count"], 7);
        assert!(output.status.success());
    }
}

#[test]
fn test_main_debug_flag_prints_env_json() {
    unsafe {
        std::env::set_var("DEBUGPREFIX_NAME", "test");

        let mut schema_file = NamedTempFile::new().unwrap();
        schema_file
            .write_all(br#"{"type":"object","properties":{"name":{"type":"string"}}}"#)
            .unwrap();
        schema_file.flush().unwrap();

        let output = Command::new(env!("CARGO_BIN_EXE_env-to-schema-json"))
            .arg("--prefix")
            .arg("DEBUGPREFIX_")
            .arg("--schema")
            .arg(schema_file.path())
            .arg("--debug")
            .output()
            .unwrap();

        std::env::remove_var("DEBUGPREFIX_NAME");

        let stdout = String::from_utf8(output.stdout).unwrap();

        assert!(stdout.contains("ENV JSON:"));
        assert!(output.status.success());
    }
}

#[test]
fn test_main_missing_schema_errors() {
    let output = Command::new(env!("CARGO_BIN_EXE_env-to-schema-json"))
        .arg("--prefix")
        .arg("NOPREFIXMATCH_")
        .stdin(Stdio::null())
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Pipe schema from stdin or provide a schema file"));
}

/// Writes a schema to a temp file that stays alive for the caller.
fn schema_file(contents: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(contents.as_bytes()).unwrap();
    file.flush().unwrap();
    file
}

#[test]
fn test_main_dashed_env_var_becomes_dashed_key() {
    // A dashed name cannot be exported from a POSIX shell, but a container
    // runtime can set one, so it is passed to the child directly here.
    let file = schema_file(
        r#"{"type":"object","properties":{
             "headers":{"type":"object","properties":{
               "set":{"type":"object"}}}}}"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_env-to-schema-json"))
        .arg("--prefix")
        .arg("DASHCLI_")
        .arg("--schema")
        .arg(file.path())
        .env(
            "DASHCLI_HEADERS_SET_X-Forwarded-For_0",
            "{http.request.header.x-real-ip}",
        )
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(
        json["headers"]["set"]["x-forwarded-for"][0],
        "{http.request.header.x-real-ip}"
    );
    assert!(output.status.success());
}

#[test]
fn test_main_double_underscore_does_not_produce_a_dash() {
    let file = schema_file(
        r#"{"type":"object","properties":{
             "headers":{"type":"object","properties":{
               "set":{"type":"object"}}}}}"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_env-to-schema-json"))
        .arg("--prefix")
        .arg("UNDERCLI_")
        .arg("--schema")
        .arg(file.path())
        .env("UNDERCLI_HEADERS_SET_X__FORWARDED__FOR_0", "value")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert!(json["headers"]["set"]["x_forwarded_for"].is_array());
    assert!(json["headers"]["set"]["x-forwarded-for"].is_null());
}

#[test]
fn test_main_indexed_array_element_is_coerced() {
    let file = schema_file(
        r#"{"type":"object","properties":{
             "ports":{"type":"array","items":{"type":"integer"}}}}"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_env-to-schema-json"))
        .arg("--prefix")
        .arg("ARRAYCLI_")
        .arg("--schema")
        .arg(file.path())
        .env("ARRAYCLI_PORTS_0", "8080")
        .env("ARRAYCLI_PORTS_1", "9090")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["ports"], serde_json::json!([8080, 9090]));
    assert!(output.status.success());
}

#[test]
fn test_main_sparse_array_exits_cleanly_naming_the_variable() {
    // Regression test: this used to abort with a Rust panic (exit 101) and a
    // backtrace that named no environment variable.
    let file = schema_file(
        r#"{"type":"object","properties":{
             "routes":{"type":"array","items":{
               "type":"object","properties":{"port":{"type":"integer"}}}}}}"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_env-to-schema-json"))
        .arg("--prefix")
        .arg("SPARSECLI_")
        .arg("--schema")
        .arg(file.path())
        .env("SPARSECLI_ROUTES_2_PORT", "8080")
        .output()
        .unwrap();

    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(!output.status.success());
    assert!(!stderr.contains("panicked"), "{}", stderr);
    assert_eq!(output.status.code(), Some(1), "{}", stderr);
    assert!(stderr.contains("SPARSECLI_ROUTES_2_PORT"), "{}", stderr);
    assert!(stderr.contains("contiguous"), "{}", stderr);
}
