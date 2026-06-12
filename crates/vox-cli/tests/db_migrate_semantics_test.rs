use std::process::Command;

fn write_minimal_schema(temp: &tempfile::TempDir) -> std::path::PathBuf {
    let schema = temp.path().join("main.vox");
    std::fs::write(&schema, "@table type Task { title: str }\n").expect("write schema");
    schema
}

#[test]
fn migrate_dry_run_uses_app_plane_when_vox_app_db_url_is_set() {
    let temp = tempfile::tempdir().expect("tempdir");
    let schema = write_minimal_schema(&temp);

    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .current_dir(temp.path())
        .env("VOX_APP_DB_URL", "not-a-real-scheme://example")
        .args([
            "db",
            "migrate",
            "--file",
            schema.to_str().expect("utf-8 schema path"),
            "--dry-run",
        ])
        .output()
        .expect("spawn vox db migrate");

    assert!(
        !out.status.success(),
        "expected app-plane URL validation failure when VOX_APP_DB_URL is set"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("failed to connect app-plane backend"),
        "expected app-plane fallback path error, got:\n{stderr}"
    );
}

#[test]
fn migrate_dry_run_defaults_to_local_codex_without_app_plane_env() {
    let temp = tempfile::tempdir().expect("tempdir");
    let schema = write_minimal_schema(&temp);

    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .current_dir(temp.path())
        .env_remove("VOX_APP_DB_URL")
        // Isolate the canonical Codex store per test process; the shared
        // user-global db file races with parallel nextest processes.
        .env("VOX_DATA_DIR", temp.path())
        .args([
            "db",
            "migrate",
            "--file",
            schema.to_str().expect("utf-8 schema path"),
            "--dry-run",
        ])
        .output()
        .expect("spawn vox db migrate");

    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("local Codex"),
        "expected local Codex dry-run output, got:\n{stdout}"
    );
}
