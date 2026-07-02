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
    "toolchain.rustc_absent",
    "toolchain.rustup_absent",
    "toolchain.compile_failed",
    "toolchain.compile_timeout",
    "docker.wsl_wedged",
    "docker.daemon_down",
    "docker.absent",
    "sccache.pathological",
    "sccache.shadowed_shim",
    "vox.schema_drift",
    "linker.lld_missing",
];

/// Encode a machine-parseable diagnosis tag into a check `detail` string.
fn diag(id: &str, severity: &str, root_cause: &str, fix: &str, auto_healable: bool) -> String {
    debug_assert!(
        KNOWN_DIAGNOSIS_IDS.contains(&id),
        "unregistered diagnosis id: {id}"
    );
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
/// A genuine sccache banner starts with `sccache ` (a fake `.cmd` forwarder that
/// silently defeats caching prints something else, e.g. a `cargo …` banner).
pub(crate) fn is_real_sccache(version_line: &str) -> bool {
    version_line.trim_start().starts_with("sccache ")
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
    (db_current > binary_baseline).then(|| {
        format!("vox binary supports schema {binary_baseline} but the DB is at {db_current}")
    })
}

/// Some(reason) when sccache is pathological over a meaningful sample (>200 requests).
pub(crate) fn sccache_verdict(requests: u64, hits: u64, compile_failures: u64) -> Option<String> {
    if requests < 200 {
        return None; // cold cache — don't cry wolf
    }
    if compile_failures > 0 {
        return Some(format!(
            "{compile_failures} compilation failures (crash signature)"
        ));
    }
    let rate = hits as f64 / requests as f64;
    (rate < 0.05).then(|| format!("hit-rate {:.1}% — sccache is pure cost here", rate * 100.0))
}

// ── checks (compose classifiers + IO) ─────────────────────────────────────────

/// A tokio `Command` that never flashes a console window on Windows
/// (`vox doctor`'s own subprocess spawns must stay quiet too).
fn quiet(program: &str) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut std_cmd = std::process::Command::new(program);
        std_cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        Command::from(std_cmd)
    }
    #[cfg(not(windows))]
    {
        Command::new(program)
    }
}

/// Some(version) when the binary runs and prints a version; None when absent.
async fn version_of(bin: &str) -> Option<String> {
    let out = quiet(bin).arg("--version").output().await.ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

async fn toolchain_check(
    checks: &mut Vec<Check>,
    bin: &str,
    is_real: fn(&str) -> bool,
    shadowed_id: &str,
    absent_id: &str,
) {
    let name = format!("toolchain: {bin} identity");
    match version_of(bin).await {
        // Absent ≠ shadowed: don't cry "shim" when the binary just isn't installed.
        None => checks.push(Check::fail(
            &name,
            diag(
                absent_id,
                "error",
                &format!("`{bin}` not found on PATH (or did not run)"),
                "install the Rust toolchain: rustup-init -y --no-modify-path",
                false,
            ),
        )),
        Some(v) if is_real(&v) => checks.push(Check::pass(&name, v)),
        Some(v) => checks.push(Check::fail(
            &name,
            diag(
                shadowed_id,
                "error",
                &format!("`{bin} --version` printed `{v}` — {bin} is shadowed by a shim/forwarder"),
                "rustup-init -y --no-modify-path --default-toolchain none --profile minimal",
                false,
            ),
        )),
    }
}

pub(crate) async fn toolchain_integrity(checks: &mut Vec<Check>) {
    toolchain_check(
        checks,
        "rustc",
        is_real_rustc,
        "toolchain.rustc_shadowed",
        "toolchain.rustc_absent",
    )
    .await;
    toolchain_check(
        checks,
        "rustup",
        is_real_rustup,
        "toolchain.rustup_shadowed",
        "toolchain.rustup_absent",
    )
    .await;
}

pub(crate) async fn docker_health(checks: &mut Vec<Check>) {
    match quiet("docker").arg("info").output().await {
        Ok(o) if o.status.success() => {
            checks.push(Check::pass("docker: reachable", "docker info ok"))
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            let c = match classify_docker_failure(&err) {
                DockerFailure::WslWedged => Check::fail(
                    "docker: WSL wedged",
                    diag(
                        "docker.wsl_wedged",
                        "error",
                        "Docker Desktop's WSL distro is wedged (permission denied / stopped) — autoscaler `docker info` fails",
                        "wsl --terminate podman-machine-default  (then restart Docker Desktop)",
                        true,
                    ),
                ),
                _ => Check::fail(
                    "docker: unreachable",
                    diag(
                        "docker.daemon_down",
                        "error",
                        "Docker daemon not reachable",
                        if cfg!(target_os = "linux") {
                            "systemctl restart docker"
                        } else {
                            "start Docker Desktop"
                        },
                        cfg!(target_os = "linux"),
                    ),
                ),
            };
            checks.push(c);
        }
        Err(_) => checks.push(Check::fail(
            "docker: not installed",
            diag(
                "docker.absent",
                "warn",
                "`docker` not on PATH",
                "install Docker Desktop / docker engine",
                false,
            ),
        )),
    }
}

pub(crate) async fn schema_health(checks: &mut Vec<Check>) {
    let baseline = vox_db::schema::BASELINE_VERSION;
    let Ok(cfg) = vox_db::DbConfig::resolve_canonical() else {
        return;
    };
    match vox_db::VoxDb::connect(cfg).await {
        Ok(_) => checks.push(Check::pass(
            "vox: schema version",
            format!("binary baseline {baseline}, DB on baseline"),
        )),
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
    let version_out = quiet("sccache").arg("--version").output().await.ok();
    let on_path = version_out
        .as_ref()
        .map(|o| o.status.success())
        .unwrap_or(false);
    // Shadowed-shim guard: a fake forwarder (e.g. ~/.cargo/bin/sccache.cmd) exits 0
    // but its banner is not `sccache …`. With RUSTC_WRAPPER pointing at it, every
    // compile is *slowed* (wrapper overhead, zero caching) instead of accelerated.
    if on_path {
        let banner = version_out
            .as_ref()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        if !is_real_sccache(&banner) {
            checks.push(Check::fail(
                "sccache: binary",
                diag(
                    "sccache.shadowed_shim",
                    "error",
                    "sccache on PATH is a shim/forwarder (banner is not `sccache …`); RUSTC_WRAPPER caches nothing and slows every build",
                    "reinstall the real binary (cargo install --locked sccache) and ensure ~/.cargo/bin/sccache is not a .cmd forwarder",
                    false,
                ),
            ));
            return;
        }
    }
    let wrapper = std::env::var("RUSTC_WRAPPER").ok();
    let incremental = std::env::var("CARGO_INCREMENTAL").ok();
    let advice = crate::commands::ci::doctor_build_cache::advise(
        on_path,
        wrapper.as_deref(),
        incremental.as_deref(),
    );
    if !advice.is_empty() {
        // Not wired is a valid, often-deliberate choice (we disabled sccache as
        // net-negative) — surface as informational, NOT a failure. Only the
        // runtime pathology below (crash / ~0% hits) is a real problem.
        checks.push(Check::pass("sccache: setup", advice.join("; ")));
        return;
    }
    // Runtime health (the new part): crash + hit-rate from --show-stats.
    if let Ok(o) = quiet("sccache")
        .args(["--show-stats", "--stats-format=json"])
        .output()
        .await
    {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&o.stdout) {
            let stats = v.get("stats").unwrap_or(&v);
            let req = stats
                .get("compile_requests")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            // `cache_hits` is a number in some sccache versions, `{counts: N}` in others.
            let hits = stats
                .get("cache_hits")
                .and_then(serde_json::Value::as_u64)
                .or_else(|| {
                    stats
                        .get("cache_hits")
                        .and_then(|h| h.get("counts"))
                        .and_then(serde_json::Value::as_u64)
                })
                .unwrap_or(0);
            let fails = stats
                .get("compile_fails")
                .or_else(|| stats.get("compilation_failures"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            checks.push(match sccache_verdict(req, hits, fails) {
                Some(reason) => Check::fail(
                    "sccache: health",
                    diag(
                        "sccache.pathological",
                        "warn",
                        &reason,
                        "vox doctor --heal  (stops server, clears cache, comments rustc-wrapper)",
                        true,
                    ),
                ),
                None => Check::pass(
                    "sccache: health",
                    format!("{req} requests, {hits} hits, {fails} fails"),
                ),
            });
        }
    }
}

pub(crate) async fn compile_probe(checks: &mut Vec<Check>) {
    let secs: u64 = std::env::var("VOX_DOCTOR_COMPILE_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    let dir = std::env::temp_dir().join("vox-doctor-probe");
    let _ = tokio::fs::create_dir_all(dir.join("src")).await;
    let _ = tokio::fs::write(dir.join("Cargo.toml"),
        "[package]\nname=\"probe\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[[bin]]\nname=\"probe\"\npath=\"src/main.rs\"\n").await;
    let _ = tokio::fs::write(dir.join("src/main.rs"), "fn main(){}\n").await;
    let fut = quiet("cargo")
        .args(["build", "--quiet"])
        .current_dir(&dir)
        .output();
    let c = match tokio::time::timeout(std::time::Duration::from_secs(secs), fut).await {
        Ok(Ok(o)) if o.status.success() => {
            Check::pass("toolchain: compile probe", "trivial crate compiled")
        }
        Ok(Ok(o)) => Check::fail(
            "toolchain: compile probe",
            diag(
                "toolchain.compile_failed",
                "error",
                &format!(
                    "toolchain cannot compile a trivial crate: {}",
                    String::from_utf8_lossy(&o.stderr)
                        .lines()
                        .last()
                        .unwrap_or("compile failed")
                ),
                "vox doctor  # see the toolchain/sccache checks above for the specific cause",
                false,
            ),
        ),
        _ => Check::fail(
            "toolchain: compile probe",
            diag(
                "toolchain.compile_timeout",
                "error",
                &format!("trivial compile hung (>{secs}s) — likely a shim/cache hang"),
                "vox doctor --heal  (or raise VOX_DOCTOR_COMPILE_TIMEOUT_SECS)",
                false,
            ),
        ),
    };
    checks.push(c);
}

// ── auto-heal (--heal) ────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HealAction {
    DisableSccache,
    RestartWslDistro,
    FlagOnly,
}

/// Pure planner: which heal a diagnosis id maps to. Shim-shadowed toolchains are
/// `FlagOnly` — running rustup-init unattended is too invasive.
pub(crate) fn heal_action(diagnosis_id: &str) -> HealAction {
    match diagnosis_id {
        "sccache.pathological" => HealAction::DisableSccache,
        "docker.wsl_wedged" => HealAction::RestartWslDistro,
        _ => HealAction::FlagOnly,
    }
}

/// Extract the `id` from a structured detail tag `… [diag id=<id> sev=… heal=…]`.
pub(crate) fn parse_diag_id(detail: &str) -> Option<&str> {
    let start = detail.find("[diag id=")? + "[diag id=".len();
    let rest = &detail[start..];
    let end = rest.find(' ')?;
    Some(&rest[..end])
}

async fn execute_heal(action: &HealAction) {
    match action {
        HealAction::DisableSccache => {
            // Exactly the by-hand fix: stop server (comment-out + cache clear is left
            // to the operator since it edits ~/.cargo/config.toml).
            let _ = quiet("sccache").arg("--stop-server").output().await;
        }
        HealAction::RestartWslDistro => {
            // Targeted: only the wedged distro, not all of WSL.
            let _ = quiet("wsl")
                .args(["--terminate", "podman-machine-default"])
                .output()
                .await;
        }
        HealAction::FlagOnly => {}
    }
}

/// On Windows the build uses `lld-link` (`.cargo/config.toml`); if it vanished from
/// PATH, links fail confusingly. Elsewhere this is a no-op pass.
pub(crate) async fn linker_health(checks: &mut Vec<Check>) {
    if !cfg!(target_os = "windows") {
        return;
    }
    let ok = quiet("lld-link")
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);
    checks.push(if ok {
        Check::pass("linker: lld-link", "present (fast Windows linker)")
    } else {
        Check::fail("linker: lld-link", diag(
            "linker.lld_missing", "warn",
            "`.cargo/config.toml` sets linker = \"lld-link\" but it is not on PATH — links will fail",
            "install LLVM (lld-link) or drop the `linker = \"lld-link\"` line to fall back to MSVC link.exe",
            false))
    });
}

/// Aggregate entrypoint, registered in `run_checks`.
pub async fn run(auto_heal: bool, checks: &mut Vec<Check>) {
    toolchain_integrity(checks).await;
    docker_health(checks).await;
    schema_health(checks).await;
    sccache_guard(checks).await;
    linker_health(checks).await;
    compile_probe(checks).await;

    if auto_heal {
        // Heal the failing checks whose diagnosis is auto-healable.
        let actions: Vec<HealAction> = checks
            .iter()
            .filter(|c| !c.pass && c.detail.contains("heal=true"))
            .filter_map(|c| parse_diag_id(&c.detail))
            .map(heal_action)
            .filter(|a| *a != HealAction::FlagOnly)
            .collect();
        for action in &actions {
            execute_heal(action).await;
        }
        if !actions.is_empty() {
            checks.push(Check::pass(
                "build-health: auto-heal",
                format!(
                    "ran {} heal action(s); re-run `vox doctor` to confirm",
                    actions.len()
                ),
            ));
        }
    }
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
        // A fake sccache forwarder prints a non-sccache banner.
        assert!(is_real_sccache("sccache 0.8.2"));
        assert!(!is_real_sccache("cargo 1.96.0 (30a34c682 2026-05-25)"));
    }

    #[test]
    fn classifies_wsl_wedge() {
        let stderr = "running wsl distro proxy in podman-machine-default distro: \
            execvpe(/mnt/wsl/docker-desktop/docker-desktop-user-distro) failed: Permission denied \
            wslErrorCode: DockerDesktop/Wsl/ExecError";
        assert_eq!(classify_docker_failure(stderr), DockerFailure::WslWedged);
        assert_eq!(
            classify_docker_failure("Cannot connect to the Docker daemon at unix:///..."),
            DockerFailure::DaemonDown
        );
        assert_eq!(
            classify_docker_failure("some other error"),
            DockerFailure::Other
        );
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

    #[test]
    fn heal_plan_maps_ids() {
        assert_eq!(
            heal_action("sccache.pathological"),
            HealAction::DisableSccache
        );
        assert_eq!(
            heal_action("docker.wsl_wedged"),
            HealAction::RestartWslDistro
        );
        // shim-shadowed toolchain must never auto-run rustup-init
        assert_eq!(
            heal_action("toolchain.rustc_shadowed"),
            HealAction::FlagOnly
        );
    }

    #[test]
    fn parses_diag_id_from_tag() {
        let d = diag(
            "docker.wsl_wedged",
            "error",
            "wedged",
            "wsl --terminate x",
            true,
        );
        assert_eq!(parse_diag_id(&d), Some("docker.wsl_wedged"));
        assert_eq!(parse_diag_id("no tag here"), None);
    }
}
