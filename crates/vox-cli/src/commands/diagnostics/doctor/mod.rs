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

    if !probe {
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

    let failed = checks.iter().filter(|c| !c.pass).count();
    if probe {
        if failed > 0 {
            anyhow::bail!("health probe: {failed} environment check(s) failed");
        }
        return Ok(());
    }

    output::print_results(&checks, test_health, json);

    Ok(())
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
