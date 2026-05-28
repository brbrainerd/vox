//! Integration tests for `vox ci ignored-test-age`, `flake-budget`, and `runtime-regress`.

mod test_governance {
    use std::fs;
    use std::process::Command;

    #[test]
    fn ignored_test_age_warn_json_smoke() {
        let out = Command::new(env!("CARGO_BIN_EXE_vox"))
            .args(["ci", "ignored-test-age", "--json"])
            .output()
            .expect("spawn vox");
        assert!(
            out.status.success(),
            "stderr={}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("\"schema_version\""),
            "expected JSON schema_version on stdout"
        );
    }

    #[test]
    fn flake_budget_enforce_respects_max_candidates_junit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let junit = dir.path().join("out.xml");
        fs::write(
            &junit,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="s">
    <testcase name="a" classname="c" time="0.1"/>
    <testcase name="a" classname="c" time="0.2"/>
  </testsuite>
</testsuites>
"#,
        )
        .expect("write junit");

        let ok = Command::new(env!("CARGO_BIN_EXE_vox"))
            .args([
                "ci",
                "flake-budget",
                "--junit",
                junit.to_str().expect("utf8"),
                "--max-candidates",
                "5",
                "--mode",
                "enforce",
            ])
            .status()
            .expect("spawn vox");
        assert!(ok.success(), "single duplicate row within budget");

        let bad = Command::new(env!("CARGO_BIN_EXE_vox"))
            .args([
                "ci",
                "flake-budget",
                "--junit",
                junit.to_str().expect("utf8"),
                "--max-candidates",
                "0",
                "--mode",
                "enforce",
            ])
            .status()
            .expect("spawn vox");
        assert!(!bad.success(), "enforce fails when max-candidates exceeded");
    }

    #[test]
    fn runtime_regress_warn_always_zero_exit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base_p = dir.path().join("base.json");
        let cur_p = dir.path().join("cur.json");
        fs::write(
            &base_p,
            r#"{
  "total_tests": 1,
  "failures": 0,
  "errors": 0,
  "skipped": 0,
  "passed": 1,
  "total_duration_secs": 1.0,
  "top_slow_tests": [{"duration_secs": 1.0, "classname": "m", "name": "t"}],
  "retry_flaky_candidates": [],
  "warnings": []
}"#,
        )
        .expect("base");
        fs::write(
            &cur_p,
            r#"{
  "total_tests": 1,
  "failures": 0,
  "errors": 0,
  "skipped": 0,
  "passed": 1,
  "total_duration_secs": 10.0,
  "top_slow_tests": [{"duration_secs": 10.0, "classname": "m", "name": "t"}],
  "retry_flaky_candidates": [],
  "warnings": []
}"#,
        )
        .expect("cur");

        let st = Command::new(env!("CARGO_BIN_EXE_vox"))
            .args([
                "ci",
                "runtime-regress",
                "--baseline",
                base_p.to_str().expect("utf8"),
                "--current",
                cur_p.to_str().expect("utf8"),
                "--mode",
                "warn",
                "--percent",
                "25",
                "--absolute-ms",
                "500",
            ])
            .status()
            .expect("spawn vox");
        assert!(st.success(), "warn mode exits 0 even with regressions");
    }

    #[test]
    fn runtime_regress_enforce_fails_on_slowdown() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base_p = dir.path().join("base.json");
        let cur_p = dir.path().join("cur.json");
        fs::write(
            &base_p,
            r#"{
  "total_tests": 1,
  "failures": 0,
  "errors": 0,
  "skipped": 0,
  "passed": 1,
  "total_duration_secs": 1.0,
  "top_slow_tests": [{"duration_secs": 1.0, "classname": "m", "name": "t"}],
  "retry_flaky_candidates": [],
  "warnings": []
}"#,
        )
        .expect("base");
        fs::write(
            &cur_p,
            r#"{
  "total_tests": 1,
  "failures": 0,
  "errors": 0,
  "skipped": 0,
  "passed": 1,
  "total_duration_secs": 10.0,
  "top_slow_tests": [{"duration_secs": 10.0, "classname": "m", "name": "t"}],
  "retry_flaky_candidates": [],
  "warnings": []
}"#,
        )
        .expect("cur");

        let st = Command::new(env!("CARGO_BIN_EXE_vox"))
            .args([
                "ci",
                "runtime-regress",
                "--baseline",
                base_p.to_str().expect("utf8"),
                "--current",
                cur_p.to_str().expect("utf8"),
                "--mode",
                "enforce",
                "--percent",
                "25",
                "--absolute-ms",
                "500",
            ])
            .status()
            .expect("spawn vox");
        assert!(!st.success(), "enforce fails on large slowdown");
    }
} // mod test_governance
