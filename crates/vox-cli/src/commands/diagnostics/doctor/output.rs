//! Doctor output formatting.

/// Print check results summary.
pub fn print_results(checks: &[super::common::Check], test_health: bool, json: bool) {
    if json {
        print_results_json(checks);
        return;
    }

    let mut failed = 0;
    for check in checks {
        if check.pass {
            println!("  ✓  {:25} {}", check.name, check.detail);
        } else {
            println!("  ✗  {:25} {}", check.name, check.detail);
            failed += 1;
        }
    }

    println!();
    if failed == 0 {
        if test_health {
            println!("✓ Test Health checks passed — automation is healthy!");
        } else {
            println!("✓ All checks passed — you're ready to build with Vox!");
        }
    } else {
        println!(
            "✗ {} check(s) failed — resolve the issues above before building.",
            failed
        );
    }
}

fn print_results_json(checks: &[super::common::Check]) {
    if let Ok(json) = serde_json::to_string_pretty(checks) {
        println!("{}", json);
    }
}

/// Single-line JSON envelope for `vox doctor --diag <id> --json`.
///
/// Mirrors the build-lane envelope contract (`crate::pipeline::BuildLaneEnvelope`):
/// the shared keys `envelope_version` / `command` / `ok` let an agent parse one
/// shape family across `vox build`/`test`/`run`/`doctor`. The payload field is
/// `checks` (doctor's own model) rather than `diagnostics` (compiler payloads),
/// and `diag_id` echoes the requested diagnosis. `ok` is `true` when the
/// requested diagnosis did NOT fire.
#[derive(serde::Serialize)]
pub(crate) struct DoctorDiagEnvelope<'a> {
    pub envelope_version: u32,
    pub command: &'static str,
    pub diag_id: &'a str,
    pub ok: bool,
    pub checks: &'a [super::common::Check],
}

/// Emit the [`DoctorDiagEnvelope`] as one compact JSONL line on stdout.
pub fn print_diag_envelope_json(diag_id: &str, ok: bool, checks: &[super::common::Check]) {
    let env = DoctorDiagEnvelope {
        envelope_version: 1,
        command: "doctor",
        diag_id,
        ok,
        checks,
    };
    if let Ok(s) = serde_json::to_string(&env) {
        println!("{s}");
    }
}

#[cfg(test)]
mod tests {
    use super::super::common::Check;
    use super::DoctorDiagEnvelope;

    #[test]
    fn diag_envelope_is_single_line_with_shared_contract_fields() {
        let checks = vec![Check::pass("linker: lld-link", "present")];
        let env = DoctorDiagEnvelope {
            envelope_version: 1,
            command: "doctor",
            diag_id: "linker.lld_missing",
            ok: true,
            checks: &checks,
        };
        let raw = serde_json::to_string(&env).expect("serialize");
        assert!(
            !raw.contains('\n'),
            "envelope must be single-line JSONL: {raw}"
        );
        let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");
        assert_eq!(v["envelope_version"], 1);
        assert_eq!(v["command"], "doctor");
        assert_eq!(v["diag_id"], "linker.lld_missing");
        assert_eq!(v["ok"], true);
        let rows = v["checks"].as_array().expect("checks array");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], "linker: lld-link");
        assert_eq!(rows[0]["pass"], true);
    }
}
