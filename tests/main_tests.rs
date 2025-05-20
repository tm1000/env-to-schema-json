use std::io::Write;
use std::process::Command;
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
