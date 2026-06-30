//! `vox doctor` build/infra health — the end-to-end truth that `--version` hides.
//!
//! `cargo --version` printed `cargo 1.96.0` while every real build aborted (shim
//! ping-pong); `rustc --version` printed `cargo …` (rustup destroyed); sccache
//! segfaulted rustc; a wedged WSL distro silently failed the autoscaler's
//! `docker info`. None surfaced. This module makes each a red, LLM-readable check.
//!
//! **Surfacing contract:** failing checks encode a machine-parseable tag in `detail`:
//! `… | FIX: <cmd> | [diag id=<id> sev=<info|warn|error> heal=<true|false>]`.
//! Agents grep `[diag id=…]` and act on `FIX:` directly. Ids are registered in
//! [`KNOWN_DIAGNOSIS_IDS`] (enforced by a test). On-demand only — zero per-build cost.

use super::super::common::Check;
use tokio::process::Command;

/// Stable diagnosis-id registry (the agent contract). Bump format ⇒ new ids here.
pub(crate) const KNOWN_DIAGNOSIS_IDS: &[&str] = &[
    "toolchain.rustc_shadowed",
    "toolchain.rustup_shadowed",
    "toolchain.compile_failed",
    "toolchain.compile_timeout",
    "docker.wsl_wedged",
    "docker.daemon_down",
    "docker.absent",
    "sccache.pathological",
    "vox.schema_drift",
];

/// Encode a machine-parseable diagnosis tag into a check `detail` string.
fn diag(id: &str, severity: &str, root_cause: &str, fix: &str, auto_healable: bool) -> String {
    debug_assert!(KNOWN_DIAGNOSIS_IDS.contains(&id), "unregistered diagnosis id: {id}");
    format!("{root_cause} | FIX: {fix} | [diag id={id} sev={severity} heal={auto_healable}]")
}

// ── pure classifiers (unit-tested) ────────────────────────────────────────────

/// A genuine rustc banner starts with `rustc ` (shadowed shims print `cargo …`).
pub(crate) fn is_real_rustc(version_line: &str) -> bool {
    version_line.trim_start().starts_with("rustc ")
}
/// A genuine rustup banner starts with `rustup ` (forwarders print `cargo …`).
pub(crate) fn is_real_rustup(version_line: &str) -> bool {
    version_line.trim_start().starts_with("rustup ")
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DockerFailure {
    WslWedged,
    DaemonDown,
    Other,
}
/// Classify a failing `docker info` stderr; recognizes the Docker-Desktop/WSL wedge.
pub(crate) fn classify_docker_failure(stderr: &str) -> DockerFailure {
    let s = stderr.to_ascii_lowercase();
    if s.contains("docker-desktop-user-distro")
        || s.contains("wslerrorcode")
        || s.contains("unexpectedly stopped")
        || (s.contains("wsl") && s.contains("permission denied"))
    {
        DockerFailure::WslWedged
    } else if s.contains("cannot connect to the docker daemon")
        || s.contains("is the docker daemon running")
    {
        DockerFailure::DaemonDown
    } else {
        DockerFailure::Other
    }
}

/// Some(detail) when the DB was migrated past what this binary understands.
pub(crate) fn schema_drift(binary_baseline: i64, db_current: i64) -> Option<String> {
    (db_current > binary_baseline)
        .then(|| format!("vox binary supports schema {binary_baseline} but the DB is at {db_current}"))
}

/// Some(reason) when sccache is pathological over a meaningful sample (>200 requests).
pub(crate) fn sccache_verdict(requests: u64, hits: u64, compile_failures: u64) -> Option<String> {
    if requests < 200 {
        return None; // cold cache — don't cry wolf
    }
    if compile_failures > 0 {
        return Some(format!("{compile_failures} compilation failures (crash signature)"));
    }
    let rate = hits as f64 / requests as f64;
    (rate < 0.05).then(|| format!("hit-rate {:.1}% — sccache is pure cost here", rate * 100.0))
}

// ── checks (compose classifiers + IO) ─────────────────────────────────────────

async fn version_of(bin: &str) -> Option<String> {
    let out = Command::new(bin).arg("--version").output().await.ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub(crate) async fn toolchain_integrity(checks: &mut Vec<Check>) {
    let rustc = version_of("rustc").await.unwrap_or_default();
    checks.push(if is_real_rustc(&rustc) {
        Check::pass("toolchain: rustc identity", &rustc)
    } else {
        Check::fail(
            "toolchain: rustc identity",
            diag("toolchain.rustc_shadowed", "error",
                &format!("`rustc --version` printed `{rustc}` — rustc is shadowed by a shim/forwarder"),
                "rustup-init -y --no-modify-path --default-toolchain none --profile minimal", false),
        )
    });
    let rustup = version_of("rustup").await.unwrap_or_default();
    checks.push(if is_real_rustup(&rustup) {
        Check::pass("toolchain: rustup identity", &rustup)
    } else {
        Check::fail(
            "toolchain: rustup identity",
            diag("toolchain.rustup_shadowed", "error",
                &format!("`rustup --version` printed `{rustup}` — rustup is missing/forwarding to cargo"),
                "rustup-init -y --no-modify-path --default-toolchain none --profile minimal", false),
        )
    });
}

pub(crate) async fn docker_health(checks: &mut Vec<Check>) {
    match Command::new("docker").arg("info").output().await {
        Ok(o) if o.status.success() => {
            checks.push(Check::pass("docker: reachable", "docker info ok"))
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            let c = match classify_docker_failure(&err) {
                DockerFailure::WslWedged => Check::fail("docker: WSL wedged", diag(
                    "docker.wsl_wedged", "error",
                    "Docker Desktop's WSL distro is wedged (permission denied / stopped) — autoscaler `docker info` fails",
                    "wsl --terminate podman-machine-default  (then restart Docker Desktop)", true)),
                _ => Check::fail("docker: unreachable", diag(
                    "docker.daemon_down", "error", "Docker daemon not reachable",
                    if cfg!(target_os = "linux") { "systemctl restart docker" } else { "start Docker Desktop" },
                    cfg!(target_os = "linux"))),
            };
            checks.push(c);
        }
        Err(_) => checks.push(Check::fail("docker: not installed", diag(
            "docker.absent", "warn", "`docker` not on PATH", "install Docker Desktop / docker engine", false))),
    }
}

pub(crate) async fn schema_health(checks: &mut Vec<Check>) {
    let baseline = vox_db::schema::BASELINE_VERSION;
    let Ok(cfg) = vox_db::DbConfig::resolve_canonical() else {
        return;
    };
    match vox_db::VoxDb::connect(cfg).await {
        Ok(_) => checks.push(Check::pass("vox: schema version", format!("binary baseline {baseline}, DB on baseline"))),
        Err(vox_db::StoreError::LegacySchemaChain { max_version }) => {
            if let Some(detail) = schema_drift(baseline, max_version) {
                checks.push(Check::fail("vox: schema drift", diag(
                    "vox.schema_drift", "error", &detail,
                    "rebuild vox if your source has the newer migration, else this DB is from a newer vox than your checkout: cargo build -p vox-cli --release",
                    false)));
            }
        }
        Err(_) => {} // other connect errors are not a schema-drift signal
    }
}

pub(crate) async fn sccache_guard(checks: &mut Vec<Check>) {
    // Reuse the existing setup advisor (do not duplicate).
    let on_path = Command::new("sccache").arg("--version").output().await.map(|o| o.status.success()).unwrap_or(false);
    let wrapper = std::env::var("RUSTC_WRAPPER").ok();
    let incremental = std::env::var("CARGO_INCREMENTAL").ok();
    let advice = crate::commands::ci::doctor_build_cache::advise(on_path, wrapper.as_deref(), incremental.as_deref());
    if !advice.is_empty() {
        checks.push(Check::new("sccache: setup", false, advice.join("; ")));
        return;
    }
    // Runtime health (the new part): crash + hit-rate from --show-stats.
    if let Ok(o) = Command::new("sccache").args(["--show-stats", "--stats-format=json"]).output().await {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&o.stdout) {
            let stats = v.get("stats").unwrap_or(&v);
            let req = stats.get("compile_requests").and_then(serde_json::Value::as_u64).unwrap_or(0);
            let hits = stats.get("cache_hits").and_then(|h| h.get("counts")).and_then(|_| None)
                .or_else(|| stats.get("cache_hits").and_then(serde_json::Value::as_u64)).unwrap_or(0);
            let fails = stats.get("compile_fails").or_else(|| stats.get("compilation_failures"))
                .and_then(serde_json::Value::as_u64).unwrap_or(0);
            checks.push(match sccache_verdict(req, hits, fails) {
                Some(reason) => Check::fail("sccache: health", diag(
                    "sccache.pathological", "warn", &reason,
                    "vox doctor --heal  (stops server, clears cache, comments rustc-wrapper)", true)),
                None => Check::pass("sccache: health", format!("{req} requests, {hits} hits, {fails} fails")),
            });
        }
    }
}

pub(crate) async fn compile_probe(checks: &mut Vec<Check>) {
    let secs: u64 = std::env::var("VOX_DOCTOR_COMPILE_TIMEOUT_SECS").ok()
        .and_then(|s| s.parse().ok()).unwrap_or(30);
    let dir = std::env::temp_dir().join("vox-doctor-probe");
    let _ = tokio::fs::create_dir_all(dir.join("src")).await;
    let _ = tokio::fs::write(dir.join("Cargo.toml"),
        "[package]\nname=\"probe\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[[bin]]\nname=\"probe\"\npath=\"src/main.rs\"\n").await;
    let _ = tokio::fs::write(dir.join("src/main.rs"), "fn main(){}\n").await;
    let fut = Command::new("cargo").args(["build", "--quiet"]).current_dir(&dir).output();
    let c = match tokio::time::timeout(std::time::Duration::from_secs(secs), fut).await {
        Ok(Ok(o)) if o.status.success() => Check::pass("toolchain: compile probe", "trivial crate compiled"),
        Ok(Ok(o)) => Check::fail("toolchain: compile probe", diag(
            "toolchain.compile_failed", "error",
            &format!("toolchain cannot compile a trivial crate: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("compile failed")),
            "vox doctor  # see the toolchain/sccache checks above for the specific cause", false)),
        _ => Check::fail("toolchain: compile probe", diag(
            "toolchain.compile_timeout", "error",
            &format!("trivial compile hung (>{secs}s) — likely a shim/cache hang"),
            "vox doctor --heal  (or raise VOX_DOCTOR_COMPILE_TIMEOUT_SECS)", false)),
    };
    checks.push(c);
}

/// Aggregate entrypoint, registered in `run_checks`.
pub async fn run(_auto_heal: bool, checks: &mut Vec<Check>) {
    toolchain_integrity(checks).await;
    docker_health(checks).await;
    schema_health(checks).await;
    sccache_guard(checks).await;
    compile_probe(checks).await;
    // Heal is staged separately (Task 9); when _auto_heal, the planner runs the
    // sccache-disable / wsl-terminate actions keyed off the emitted diagnosis ids.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_shadowed_rustc() {
        assert!(!is_real_rustc("cargo 1.96.0 (30a34c682 2026-05-25)"));
        assert!(is_real_rustc("rustc 1.96.0 (ac68faa20 2026-05-25)"));
        assert!(!is_real_rustup("cargo 1.96.0"));
        assert!(is_real_rustup("rustup 1.29.0 (28d1352db 2026-03-05)"));
    }

    #[test]
    fn classifies_wsl_wedge() {
        let stderr = "running wsl distro proxy in podman-machine-default distro: \
            execvpe(/mnt/wsl/docker-desktop/docker-desktop-user-distro) failed: Permission denied \
            wslErrorCode: DockerDesktop/Wsl/ExecError";
        assert_eq!(classify_docker_failure(stderr), DockerFailure::WslWedged);
        assert_eq!(classify_docker_failure("Cannot connect to the Docker daemon at unix:///..."), DockerFailure::DaemonDown);
        assert_eq!(classify_docker_failure("some other error"), DockerFailure::Other);
    }

    #[test]
    fn flags_schema_drift() {
        assert!(schema_drift(80, 81).is_some());
        assert!(schema_drift(81, 81).is_none());
        assert!(schema_drift(81, 80).is_none());
    }

    #[test]
    fn flags_sccache_pathology() {
        assert!(sccache_verdict(300, 1, 5).is_some()); // crash
        assert!(sccache_verdict(300, 1, 0).is_some()); // 0% hits
        assert!(sccache_verdict(300, 270, 0).is_none()); // healthy
        assert!(sccache_verdict(10, 0, 0).is_none()); // too small a sample
    }

    #[test]
    fn diag_ids_registered_and_unique() {
        assert!(!KNOWN_DIAGNOSIS_IDS.is_empty());
        let set: std::collections::HashSet<_> = KNOWN_DIAGNOSIS_IDS.iter().collect();
        assert_eq!(set.len(), KNOWN_DIAGNOSIS_IDS.len());
    }
}
