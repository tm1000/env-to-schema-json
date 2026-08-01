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
