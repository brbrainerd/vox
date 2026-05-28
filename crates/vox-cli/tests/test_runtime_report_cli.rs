//! Integration smoke: `vox ci test-runtime-report` reads JUnit XML end-to-end.

use std::fs;
use std::process::Command;

#[test]
fn test_runtime_report_cli_json_stdout() {
    let dir = tempfile::tempdir().expect("tempdir");
    let junit = dir.path().join("out.xml");
    fs::write(
        &junit,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="crate::it" tests="1">
    <testcase name="smoke" classname="crate::it" time="0.42" file="crates/demo/tests/it.rs"/>
  </testsuite>
</testsuites>
"#,
    )
    .expect("write junit");

    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args([
            "ci",
            "test-runtime-report",
            "--junit",
            junit.to_str().expect("utf8 path"),
            "--json",
            "--top",
            "5",
        ])
        .output()
        .expect("spawn vox");

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("\"total_tests\": 1"));
    assert!(stdout.contains("smoke"));
}

#[test]
fn test_runtime_report_writes_markdown() {
    let dir = tempfile::tempdir().expect("tempdir");
    let junit = dir.path().join("out.xml");
    fs::write(
        &junit,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="t">
    <testcase name="a" classname="c" time="1"/>
  </testsuite>
</testsuites>
"#,
    )
    .expect("write junit");
    let md_path = dir.path().join("report.md");

    let st = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args([
            "ci",
            "test-runtime-report",
            "--junit",
            junit.to_str().expect("utf8 path"),
            "--json",
            "--markdown",
            md_path.to_str().expect("utf8 path"),
        ])
        .status()
        .expect("spawn vox");
    assert!(st.success());
    let md = fs::read_to_string(&md_path).expect("read md");
    assert!(md.contains("Test runtime report"));
    assert!(md.contains("a"));
}
