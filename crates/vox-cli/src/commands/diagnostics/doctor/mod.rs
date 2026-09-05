//! `vox doctor` — check the development environment is ready.

#[cfg(feature = "codex")]
mod checks_codex;
mod checks_standard;
mod common;
mod output;
pub mod project_check;
mod provider_policy;

use anyhow::Result;

/// Run the `vox doctor` environment check and health audit.
pub async fn run(
    compile_target: Option<&str>,
    auto_heal: bool,
    test_health: bool,
    build_perf: bool,
    scope: bool,
    json: bool,
    probe: bool,
    fix_cuda_path: bool,
    tier: &str,
    diag: Option<&str>,
) -> Result<()> {
    if let Some(id) = diag {
        if probe || build_perf || scope || test_health || auto_heal || compile_target.is_some() {
            anyhow::bail!(
                "`--diag` runs a single build-health check and cannot be combined with \
                 --probe, --build-perf, --scope, --test-health, --auto-heal, or --compile-target"
            );
        }
        let mut checks: Vec<common::Check> = Vec::new();
        if !checks_standard::run_diag_check(id, &mut checks).await {
            anyhow::bail!(
                "unknown diag id `{id}` — known ids:\n  {}",
                checks_standard::known_diag_ids().join("\n  ")
            );
        }
        let fired = checks
            .iter()
            .any(|c| !c.pass && checks_standard::parse_diag_id(&c.detail) == Some(id));
        if json {
            // Single-line envelope sharing the build-lane `--json` contract keys
            // (envelope_version/command/ok) so agents parse one shape family across
            // the CLI; `ok` is true when the requested diagnosis did not fire.
            output::print_diag_envelope_json(id, !fired, &checks);
        } else {
            output::print_results(&checks, false, json);
        }
        if fired {
            anyhow::bail!("doctor: diagnosis `{id}` fired — apply the FIX above and re-run");
        }
        return Ok(());
    }

    #[cfg(not(feature = "codex"))]
    if build_perf || scope || json {
        anyhow::bail!(
            "`vox doctor` with --build-perf, --scope, or --json requires the extended build: \
             `cargo build -p vox-cli --features codex` (wires `commands::diagnostics::doctor`)."
        );
    }

    if fix_cuda_path {
        run_fix_cuda_path()?;
        return Ok(());
    }

    if probe {
        if build_perf || scope || json {
            anyhow::bail!("`--probe` cannot be combined with --build-perf, --scope, or --json");
        }
        if auto_heal || test_health {
            anyhow::bail!("`--probe` cannot be combined with --auto-heal or --test-health");
        }
    }

    if !probe && !json {
        println!(
            "vox doctor — checking your environment{}",
            if auto_heal {
                " (auto-healing enabled)"
            } else {
                ""
            }
        );
        println!();
    }

    let mut checks: Vec<common::Check> = Vec::new();

    #[cfg(feature = "codex")]
    if build_perf {
        checks_codex::run_build_perf(json).await?;
        return Ok(());
    }

    #[cfg(feature = "codex")]
    if scope {
        checks_codex::run_scope(json).await?;
        return Ok(());
    }

    checks_standard::run_checks(auto_heal, test_health, compile_target, tier, &mut checks).await;

    if probe {
        let failed = failed_probe_required(&checks);
        if !probe_verdict(&checks) {
            anyhow::bail!(
                "health probe: {} required environment check(s) failed: {}",
                failed.len(),
                failed.join(", ")
            );
        }
        return Ok(());
    }

    output::print_results(&checks, test_health, json);

    Ok(())
}

// ── `--probe` required-check subset ───────────────────────────────────────────

/// Checks whose failure means *this binary is not functional*. `--probe` is the
/// container HEALTHCHECK (`Dockerfile`: `vox doctor --probe`), and the runtime
/// image has no repo, no `Vox.toml` and no API keys — so gating on "any check
/// failed" made the container permanently unhealthy. Everything repo-scoped
/// (`Vox.toml`, `Vox Config`, `Workspace Registration`, `vox-lsp binary`),
/// credential-scoped (`Google AI Studio Key`, …) or tier-optional (`tier dep: …`)
/// is advisory and must NOT sink the probe.
///
/// `docker: not installed` is deliberately absent: Docker is only required
/// *if applicable*, and a runtime container legitimately has no docker CLI. A
/// docker daemon that is present but unreachable IS a real failure.
const PROBE_REQUIRED_CHECKS: &[&str] = &[
    // Docker reachability, but only when docker is actually installed.
    "docker: unreachable",
    "docker: WSL wedged",
    // VoxDB data directory must be writable or nothing the binary does persists.
    "VoxDB directory",
    // Schema must match the binary's baseline (pass name / drift-failure name).
    "vox: schema version",
    "vox: schema drift",
];

/// A check is required when it is named in [`PROBE_REQUIRED_CHECKS`].
///
/// Toolchain identity rows are deliberately **not** required. `--probe` is the
/// container HEALTHCHECK (`Dockerfile`), and the runtime image is a slim Debian
/// carrying only the `vox` binary — it has no Rust toolchain at all, so requiring
/// `toolchain: rustc identity` would keep every container permanently unhealthy,
/// which is the bug this required-subset exists to fix. A shadowed toolchain is a
/// real problem on a developer machine, and `vox doctor` still reports it there;
/// it just is not a statement about whether a shipped binary is functional.
fn is_probe_required(name: &str) -> bool {
    PROBE_REQUIRED_CHECKS.contains(&name)
}

/// Names of the *required* checks that are currently failing.
fn failed_probe_required(checks: &[common::Check]) -> Vec<&str> {
    checks
        .iter()
        .filter(|c| !c.pass && is_probe_required(&c.name))
        .map(|c| c.name.as_str())
        .collect()
}

/// Pure `--probe` verdict: `true` = healthy. Kept I/O-free so it is unit-testable.
fn probe_verdict(checks: &[common::Check]) -> bool {
    failed_probe_required(checks).is_empty()
}

fn run_fix_cuda_path() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let script = r#"
            $ErrorActionPreference = 'Stop'
            $CudaRoot = Join-Path $env:ProgramW6432 'NVIDIA GPU Computing Toolkit\CUDA\v13.1'
            $bin = Join-Path $CudaRoot 'bin'
            $binX64 = Join-Path $CudaRoot 'bin\x64'
            if (-not (Test-Path (Join-Path $bin 'nvcc.exe'))) {
                Write-Error "nvcc.exe not found under $bin — adjust -CudaRoot or install CUDA Toolkit."
            }
            $toAdd = @($bin, $binX64) | Where-Object { Test-Path $_ }
            $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
            if (-not $userPath) { $userPath = '' }
            $parts = $userPath -split ';' | Where-Object { $_ -ne '' }
            $missing = $toAdd | Where-Object { $parts -notcontains $_ }
            if ($missing.Count -eq 0) {
                Write-Host "User PATH already contains CUDA bin entries for $CudaRoot"
            } else {
                $newPath = ($toAdd + $parts) -join ';'
                [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
                Write-Host "Updated User PATH (prepended): $($missing -join '; ')"
            }
            [Environment]::SetEnvironmentVariable('CUDA_PATH', $CudaRoot, 'User')
            Write-Host "Set User CUDA_PATH=$CudaRoot"
            Write-Host "Open a **new** terminal (or restart) so processes pick up the change."
        "#;
        // vox-arch-check: allow shell-spawn
        std::process::Command::new("pwsh")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(script)
            .status()?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!("--fix-cuda-path is only supported on Windows.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::Check;

    /// The checks a *runtime container* (no repo, no Vox.toml, no API keys) always
    /// fails. None of them says the binary is broken, so none may sink the probe.
    fn container_advisory_failures() -> Vec<Check> {
        vec![
            Check::fail("Vox.toml", "no Vox.toml in cwd"),
            Check::fail("Vox Config", "not loadable"),
            Check::fail("Google AI Studio Key", "missing"),
            Check::fail("vox-lsp binary", "not built"),
            Check::fail("Workspace Registration", "not registered"),
            Check::fail("tier dep: ffmpeg", "not found"),
            Check::fail("tier dep: onnxruntime", "not found"),
            Check::fail("docker: not installed", "`docker` not on PATH"),
        ]
    }

    fn required_passing() -> Vec<Check> {
        vec![
            Check::pass("toolchain: rustc identity", "rustc 1.96.0"),
            Check::pass("toolchain: rustup identity", "rustup 1.29.0"),
            Check::pass("docker: reachable", "docker info ok"),
            Check::pass("VoxDB directory", "/root/.vox (writable)"),
            Check::pass("vox: schema version", "binary baseline 42, DB on baseline"),
        ]
    }

    #[test]
    fn probe_ignores_advisory_failures() {
        let mut checks = required_passing();
        checks.extend(container_advisory_failures());
        assert!(
            probe_verdict(&checks),
            "advisory failures must not sink the probe; failed required: {:?}",
            failed_probe_required(&checks)
        );
    }

    /// The runtime image (`Dockerfile`) is a slim Debian carrying only the `vox`
    /// binary — no Rust toolchain. Requiring toolchain identity would keep every
    /// container permanently unhealthy, which is the exact bug the required-subset
    /// exists to fix. `vox doctor` still surfaces a shadowed toolchain on a dev box.
    #[test]
    fn probe_ignores_a_missing_toolchain() {
        for advisory in [
            Check::fail("toolchain: rustc identity", "not found on PATH"),
            Check::fail("toolchain: rustup identity", "not found on PATH"),
            Check::fail("toolchain: compile probe", "cargo absent"),
        ] {
            let name = advisory.name.clone();
            let mut checks = required_passing();
            checks.push(advisory);
            assert!(
                probe_verdict(&checks),
                "a failing `{name}` must not sink the container health probe"
            );
        }
    }

    #[test]
    fn probe_fails_on_required_check_failure() {
        for bad in [
            Check::fail("docker: unreachable", "Docker daemon not reachable"),
            Check::fail("docker: WSL wedged", "WSL2 Docker Engine unreachable"),
            Check::fail("VoxDB directory", "~/.vox/ not writable"),
            Check::fail("vox: schema drift", "DB ahead of binary baseline"),
        ] {
            let name = bad.name.clone();
            let mut checks = required_passing();
            checks.extend(container_advisory_failures());
            checks.push(bad);
            assert!(
                !probe_verdict(&checks),
                "a failing `{name}` must make the probe unhealthy"
            );
            assert!(
                failed_probe_required(&checks).contains(&name.as_str()),
                "`{name}` should be reported as a failed required check"
            );
        }
    }

    /// An all-green environment is healthy, and an empty check list is not
    /// "unhealthy" by accident.
    #[test]
    fn probe_healthy_when_all_required_pass() {
        assert!(probe_verdict(&required_passing()));
        assert!(probe_verdict(&[]));
    }

    #[tokio::test]
    #[cfg(not(feature = "codex"))]
    async fn extended_doctor_flags_require_codex_build() {
        let err = run(
            None, false, false, true, false, false, false, false, "full", None,
        )
        .await
        .expect_err("build_perf without codex doctor should error");
        let s = err.to_string();
        assert!(
            s.contains("codex") && s.contains("doctor"),
            "unexpected message: {s}"
        );
    }

    #[tokio::test]
    #[cfg(feature = "codex")]
    async fn build_perf_runs_when_codex_enabled() {
        let r = run(
            None, false, false, true, false, false, false, false, "full", None,
        )
        .await;
        assert!(r.is_ok(), "expected build_perf path to complete: {r:?}");
    }

    #[tokio::test]
    async fn diag_flag_rejects_unknown_id() {
        let err = run(
            None,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            "full",
            Some("bogus.id"),
        )
        .await
        .expect_err("unknown diag id should error");
        let s = err.to_string();
        assert!(s.contains("unknown diag id"), "unexpected message: {s}");
        assert!(
            s.contains("sccache.pathological"),
            "error should list the known-id registry: {s}"
        );
    }
}
