# Build/Infra Health Doctor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Make build/infra failures (toolchain-shim shadowing, Docker/WSL wedge, sccache crash, schema-version drift) surface loudly and machine-readably in `vox doctor`, self-heal what's safe, and stop the scheduled task from emitting ANSI garbage or flashing windows.

**Architecture:** One new on-demand `vox doctor` check module (`build_health.rs`) with cheap-first layers, plus two targeted fixes outside it (TTY-aware logging; hidden/quiet task + spawns). Detection is on-demand only — zero per-build overhead. Structured `Diagnosis` fields make every finding LLM-actionable.

**Tech Stack:** Rust (`vox-cli` doctor, `vox-foundation` tracing, `vox-db` schema), `std::io::IsTerminal`, sccache JSON stats, Windows Task Scheduler XML, `CREATE_NO_WINDOW`.

**Spec:** `docs/superpowers/specs/2026-06-29-build-infra-health-doctor-design.md` (sccache *acceleration* is the separate `…-sccache-acceleration-design.md`).

---

## File Structure

| File | Change |
|---|---|
| `crates/vox-foundation/src/tracing.rs` | TTY-aware `with_ansi` on all `fmt()` builders (fixes ANSI garbage) |
| `crates/vox-cli/src/commands/diagnostics/doctor/common.rs` | add optional `Diagnosis` fields to `Check` |
| `…/doctor/checks_standard/build_health.rs` | **new** — toolchain integrity, Docker/WSL, schema-drift, sccache guard, compile probe |
| `…/doctor/checks_standard/mod.rs` | register `build_health::run` |
| `…/doctor/heal.rs` (or the existing auto-heal path) | heal actions |
| `crates/vox-db/src/schema.rs` | `pub fn db_schema_version(path) -> u32` helper (if not already public) |
| `scripts/ci/voxcirunnerscale.task.xml` | `<Hidden>true</Hidden>` |
| `crates/vox-cli/src/commands/ci/runner_scale.rs` + watchdog spawns | route through `quiet_command` |

---

## Task 1: TTY-aware logging — kill the ANSI garbage in non-terminals

**Files:** Modify `crates/vox-foundation/src/tracing.rs`. Test: same file's test module.

- [ ] **Step 1: Failing test** — assert color is off when not a terminal:
```rust
#[test]
fn ansi_disabled_when_not_a_tty() {
    // Under `cargo test` stdout/stderr are not terminals, so the helper must pick ansi=false.
    assert!(!super::ansi_enabled_for(/*is_terminal=*/ false));
    assert!(super::ansi_enabled_for(true));
}
```

- [ ] **Step 2: Run it, confirm FAIL** (`cargo test -p vox-foundation ansi_disabled_when_not_a_tty` → `ansi_enabled_for` undefined).

- [ ] **Step 3: Implement** — add the helper and thread it through every builder:
```rust
use std::io::IsTerminal;

/// ANSI only when the sink is a real terminal — scheduled tasks / pipes get clean text.
pub(crate) fn ansi_enabled_for(is_terminal: bool) -> bool { is_terminal }

pub fn try_init_cli_default_info_fallback() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(ansi_enabled_for(std::io::stdout().is_terminal()))
        .try_init();
}

pub fn try_init_from_default_env() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_ansi(ansi_enabled_for(std::io::stdout().is_terminal()))
        .try_init();
}

pub fn try_init_from_default_env_stderr() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(ansi_enabled_for(std::io::stderr().is_terminal()))
        .try_init();
}
```

- [ ] **Step 4: Run tests, confirm PASS** (`cargo test -p vox-foundation`).

- [ ] **Step 5: Verify the real symptom is gone** — run the scheduled-task command with stdout redirected (non-TTY) and confirm no `[2m`/`[0m`:
```bash
RUST_LOG=info vox run scripts/ci-runners-up.vox > /tmp/tick.out 2>&1; grep -c $'\x1b\[' /tmp/tick.out   # expect 0
```

- [ ] **Step 6: Commit** `fix(logging): disable ANSI color when stdout/stderr is not a TTY`.

---

## Task 2: LLM-readable `Diagnosis` fields on `Check`

**Files:** Modify `crates/vox-cli/src/commands/diagnostics/doctor/common.rs`. Test: inline.

- [ ] **Step 1: Failing test** — a failing check serializes with the structured fields:
```rust
#[test]
fn check_emits_diagnosis_json() {
    let c = Check::fail("toolchain.rustc_shadowed", "rustc prints cargo version")
        .with_diagnosis("toolchain.rustc_shadowed", Severity::Error,
            "rustc resolves to a cargo forwarder",
            "rustup-init -y --no-modify-path --default-toolchain none", false);
    let v = serde_json::to_value(&c).unwrap();
    assert_eq!(v["diagnosis"]["remediation_command"], "rustup-init -y --no-modify-path --default-toolchain none");
    assert_eq!(v["diagnosis"]["auto_healable"], false);
}
```

- [ ] **Step 2: Run it, confirm FAIL** (`with_diagnosis`/`Severity` undefined).

- [ ] **Step 3: Implement** — additive (existing `name/pass/detail` unchanged, back-compat):
```rust
#[derive(Debug, Serialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Severity { Info, Warn, Error }

#[derive(Debug, Serialize)]
pub(crate) struct Diagnosis {
    pub id: String,
    pub severity: Severity,
    pub root_cause: String,
    pub remediation_command: String,
    pub auto_healable: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct Check {
    pub name: String,
    pub pass: bool,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnosis: Option<Diagnosis>,
}
```
Add `diagnosis: None` to `pass`/`fail`/`new`, and a builder:
```rust
pub(crate) fn with_diagnosis(mut self, id: impl Into<String>, severity: Severity,
    root_cause: impl Into<String>, remediation_command: impl Into<String>, auto_healable: bool) -> Self {
    self.diagnosis = Some(Diagnosis { id: id.into(), severity,
        root_cause: root_cause.into(), remediation_command: remediation_command.into(), auto_healable });
    self
}
```

- [ ] **Step 4: Run tests, confirm PASS** (`cargo test -p vox-cli check_emits_diagnosis_json`).

- [ ] **Step 5: Commit** `feat(doctor): structured Diagnosis fields for LLM-readable --json`.

---

## Task 3: build_health — toolchain integrity (rustc/rustup/cargo identity)

**Files:** Create `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/build_health.rs`. Test: inline.

- [ ] **Step 1: Failing test** — the classifier flags a shadowed rustc:
```rust
#[test]
fn classifies_shadowed_rustc() {
    // rustc that prints a cargo banner is the shim-shadow signature we hit.
    assert!(!is_real_rustc("cargo 1.96.0 (30a34c682 2026-05-25)"));
    assert!(is_real_rustc("rustc 1.96.0 (ac68faa20 2026-05-25)"));
}
```

- [ ] **Step 2: Run it, confirm FAIL** (`is_real_rustc` undefined).

- [ ] **Step 3: Implement** the classifier + check builder:
```rust
use crate::commands::diagnostics::doctor::common::{Check, Severity};
use tokio::process::Command;

pub(crate) fn is_real_rustc(version_line: &str) -> bool { version_line.trim_start().starts_with("rustc ") }
pub(crate) fn is_real_rustup(version_line: &str) -> bool { version_line.trim_start().starts_with("rustup ") }

async fn version_of(bin: &str) -> Option<String> {
    let out = Command::new(bin).arg("--version").output().await.ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub(crate) async fn toolchain_integrity(checks: &mut Vec<Check>) {
    let rustc = version_of("rustc").await.unwrap_or_default();
    checks.push(if is_real_rustc(&rustc) {
        Check::pass("toolchain: rustc identity", &rustc)
    } else {
        Check::fail("toolchain: rustc identity", format!("`rustc --version` printed `{rustc}` — rustc is shadowed by a shim/forwarder"))
            .with_diagnosis("toolchain.rustc_shadowed", Severity::Error,
                "rustc resolves to a cargo forwarder (rustup proxies clobbered)",
                "rustup-init -y --no-modify-path --default-toolchain none --profile minimal", false)
    });
    let rustup = version_of("rustup").await.unwrap_or_default();
    checks.push(if is_real_rustup(&rustup) {
        Check::pass("toolchain: rustup identity", &rustup)
    } else {
        Check::fail("toolchain: rustup identity", format!("`rustup --version` printed `{rustup}` — rustup is missing/forwarding to cargo"))
            .with_diagnosis("toolchain.rustup_shadowed", Severity::Error,
                "rustup proxy destroyed; cargo/rustc/rustup all forward to cargo",
                "rustup-init -y --no-modify-path --default-toolchain none --profile minimal", false)
    });
}
```

- [ ] **Step 4: Run tests, confirm PASS** (`cargo test -p vox-cli classifies_shadowed_rustc`).

- [ ] **Step 5: Commit** `feat(doctor): toolchain integrity check (detects shim-shadowed rustc/rustup)`.

---

## Task 4: build_health — Docker/WSL reachability + wedge classifier

**Files:** Modify `build_health.rs`. Test: inline.

- [ ] **Step 1: Failing test** — the exact WSL stderr we hit classifies as a wedge:
```rust
#[test]
fn classifies_wsl_wedge() {
    let stderr = "running wsl distro proxy in podman-machine-default distro: ... \
        execvpe(/mnt/wsl/docker-desktop/docker-desktop-user-distro) failed: Permission denied ... \
        wslErrorCode: DockerDesktop/Wsl/ExecError";
    assert_eq!(classify_docker_failure(stderr), DockerFailure::WslWedged);
    assert_eq!(classify_docker_failure("Cannot connect to the Docker daemon at ..."), DockerFailure::DaemonDown);
}
```

- [ ] **Step 2: Run it, confirm FAIL** (`classify_docker_failure`/`DockerFailure` undefined).

- [ ] **Step 3: Implement**:
```rust
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DockerFailure { WslWedged, DaemonDown, Ok }

pub(crate) fn classify_docker_failure(stderr: &str) -> DockerFailure {
    let s = stderr.to_ascii_lowercase();
    if s.contains("docker-desktop-user-distro") || s.contains("wslerrorcode")
        || (s.contains("wsl") && s.contains("permission denied")) || s.contains("unexpectedly stopped") {
        DockerFailure::WslWedged
    } else if s.contains("cannot connect to the docker daemon") || s.contains("is the docker daemon running") {
        DockerFailure::DaemonDown
    } else { DockerFailure::Ok }
}

pub(crate) async fn docker_health(checks: &mut Vec<Check>) {
    let out = Command::new("docker").args(["info"]).output().await;
    match out {
        Ok(o) if o.status.success() => checks.push(Check::pass("docker: reachable", "docker info ok")),
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            match classify_docker_failure(&err) {
                DockerFailure::WslWedged => checks.push(
                    Check::fail("docker: WSL wedged", "Docker Desktop's WSL distro is wedged (permission denied / stopped)")
                        .with_diagnosis("docker.wsl_wedged", Severity::Error,
                            "podman-machine-default WSL distro proxy lost exec permission — autoscaler's `docker info` precondition fails silently",
                            "wsl --terminate podman-machine-default  (then restart Docker Desktop)", true)),
                _ => checks.push(
                    Check::fail("docker: unreachable", "docker daemon not reachable")
                        .with_diagnosis("docker.daemon_down", Severity::Error, "Docker daemon down",
                            if cfg!(target_os="linux") {"systemctl restart docker"} else {"start Docker Desktop"}, cfg!(target_os="linux"))),
            }
        }
        Err(_) => checks.push(Check::fail("docker: not installed", "`docker` not on PATH")
            .with_diagnosis("docker.absent", Severity::Warn, "docker CLI missing", "install Docker Desktop / docker engine", false)),
    }
}
```

- [ ] **Step 4: Run tests, confirm PASS**.

- [ ] **Step 5: Commit** `feat(doctor): Docker/WSL reachability check with wedge classifier`.

---

## Task 5: build_health — schema-version drift (binary vs DB)

**Files:** Modify `build_health.rs`; if needed add `pub fn db_schema_version` in `crates/vox-db/src/schema.rs`. Test: inline classifier test.

- [ ] **Step 1: Failing test** — drift is flagged when DB schema exceeds the binary baseline:
```rust
#[test]
fn flags_schema_drift() {
    assert!(schema_drift(/*binary_baseline=*/80, /*db_current=*/81).is_some());
    assert!(schema_drift(81, 81).is_none());
}
```

- [ ] **Step 2: Run it, confirm FAIL** (`schema_drift` undefined).

- [ ] **Step 3: Implement** — surface the exact `store/open.rs:117` condition as a real check:
```rust
/// Some(detail) when the DB was migrated past what this binary understands.
pub(crate) fn schema_drift(binary_baseline: u32, db_current: u32) -> Option<String> {
    (db_current > binary_baseline).then(|| format!(
        "vox binary supports schema {binary_baseline} but the DB is at {db_current}"))
}

pub(crate) fn schema_health(checks: &mut Vec<Check>) {
    let baseline = vox_db::schema::BASELINE_VERSION;
    let Some(db_current) = vox_db::schema::db_schema_version(&vox_db::paths::canonical_db_path()) else { return };
    match schema_drift(baseline, db_current) {
        None => checks.push(Check::pass("vox: schema version", format!("binary {baseline} == db {db_current}"))),
        Some(detail) => checks.push(Check::fail("vox: schema drift", detail)
            .with_diagnosis("vox.schema_drift", Severity::Error,
                "the installed vox binary is older than the binary that migrated this DB (its source branch is behind the migration that bumped the DB)",
                "rebuild vox from a branch including the newer migration: cargo build -p vox-cli --release  (then install)", false)),
    }
}
```
(If `db_schema_version` is not already public, add it: a `SELECT MAX(version) FROM schema_version` over a read-only connection, mirroring `store/open.rs`'s `current_version`.)

- [ ] **Step 4: Run tests, confirm PASS** (`cargo test -p vox-cli flags_schema_drift`).

- [ ] **Step 5: Commit** `feat(doctor): surface vox-binary-vs-DB schema drift (the silent upgrade-path warning)`.

---

## Task 6: build_health — sccache guard (read-only stats)

**Files:** Modify `build_health.rs`. Test: inline parser test.

- [ ] **Step 1: Failing test** — parse stats + flag the pathologies:
```rust
#[test]
fn flags_sccache_pathology() {
    // crash signature
    assert!(sccache_verdict(/*requests=*/300, /*hits=*/1, /*compile_failures=*/5).is_some());
    // 0%-hit signature over a meaningful sample
    assert!(sccache_verdict(300, 1, 0).is_some());
    // healthy
    assert!(sccache_verdict(300, 270, 0).is_none());
    // too small a sample → no verdict (don't cry wolf on a cold cache)
    assert!(sccache_verdict(10, 0, 0).is_none());
}
```

- [ ] **Step 2: Run it, confirm FAIL** (`sccache_verdict` undefined).

- [ ] **Step 3: Implement** (only runs when sccache is the configured wrapper):
```rust
/// Some(reason) when sccache is pathological over a meaningful sample (>200 requests).
pub(crate) fn sccache_verdict(requests: u64, hits: u64, compile_failures: u64) -> Option<String> {
    if requests < 200 { return None; }
    if compile_failures > 0 { return Some(format!("{compile_failures} compilation failures (crash signature)")); }
    let rate = hits as f64 / requests as f64;
    (rate < 0.05).then(|| format!("hit-rate {:.1}% — sccache is pure cost here", rate * 100.0))
}
```
The check reads `sccache --show-stats --stats-format=json`, extracts the three numbers, and on `Some` emits a failing `Check` with `auto_healable: true` (heal = disable+clear, Task 9). When sccache is *not* the wrapper, push a passing informational check.

- [ ] **Step 4: Run tests, confirm PASS**.

- [ ] **Step 5: Commit** `feat(doctor): sccache health guard (crash + 0%-hit detection)`.

---

## Task 7: build_health — real compile probe (the keystone)

**Files:** Modify `build_health.rs`. Test: integration (tempdir).

- [ ] **Step 1: Failing test** — a broken wrapper yields a red probe; a healthy toolchain green:
```rust
#[tokio::test]
async fn compile_probe_detects_broken_wrapper() {
    // RUSTC_WRAPPER=false makes every rustc invocation fail — must be caught.
    let bad = compile_probe(Some("false")).await;
    assert!(!bad.pass);
    let good = compile_probe(None).await; // as-configured
    assert!(good.pass, "healthy toolchain must compile a trivial crate");
}
```

- [ ] **Step 2: Run it, confirm FAIL** (`compile_probe` undefined).

- [ ] **Step 3: Implement** — write a trivial crate to a temp dir, `cargo build` with a ~30s timeout through the as-configured toolchain:
```rust
pub(crate) async fn compile_probe(force_wrapper: Option<&str>) -> Check {
    let dir = std::env::temp_dir().join("vox-doctor-compile-probe");
    let _ = tokio::fs::create_dir_all(dir.join("src")).await;
    let _ = tokio::fs::write(dir.join("Cargo.toml"),
        "[package]\nname=\"probe\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[[bin]]\nname=\"probe\"\npath=\"src/main.rs\"\n").await;
    let _ = tokio::fs::write(dir.join("src/main.rs"), "fn main(){}\n").await;
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--quiet"]).current_dir(&dir);
    if let Some(w) = force_wrapper { cmd.env("RUSTC_WRAPPER", w); }
    let run = tokio::time::timeout(std::time::Duration::from_secs(30), cmd.output()).await;
    match run {
        Ok(Ok(o)) if o.status.success() => Check::pass("toolchain: compile probe", "trivial crate compiled"),
        Ok(Ok(o)) => Check::fail("toolchain: compile probe", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("compile failed").to_string())
            .with_diagnosis("toolchain.compile_failed", Severity::Error,
                "the toolchain cannot compile a trivial crate (shim ping-pong, sccache segfault, or broken rustc)",
                "vox doctor  # see the toolchain integrity + sccache checks above for the specific cause", false),
        _ => Check::fail("toolchain: compile probe", "compile probe timed out (>30s) — likely a hung shim/cache")
            .with_diagnosis("toolchain.compile_timeout", Severity::Error, "trivial compile hung", "vox doctor --heal", false),
    }
}
```

- [ ] **Step 4: Run tests, confirm PASS** (`cargo test -p vox-cli compile_probe_detects_broken_wrapper -- --include-ignored` if gated).

- [ ] **Step 5: Commit** `feat(doctor): real compile probe — the end-to-end truth --version hides`.

---

## Task 8: Register build_health in the doctor registry

**Files:** Modify `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/mod.rs`.

- [ ] **Step 1: Wire it** following the existing `toolchain::run(...)` call pattern — assemble the layers:
```rust
pub(crate) async fn run(auto_heal: bool, checks: &mut Vec<Check>) {
    build_health::toolchain_integrity(checks).await;
    build_health::docker_health(checks).await;
    build_health::schema_health(checks);
    build_health::sccache_guard(checks).await;
    checks.push(build_health::compile_probe(None).await);
    if auto_heal { build_health::heal(checks).await; }   // Task 9
}
```
and add `mod build_health;` + the call from the standard checks aggregator.

- [ ] **Step 2: Verify** `cargo build -p vox-cli` compiles and `vox doctor --json | jq '.[].diagnosis.id'` shows the new ids on a healthy machine (all pass).

- [ ] **Step 3: Commit** `feat(doctor): register build_health check module`.

---

## Task 9: Auto-heal actions (`vox doctor --heal`)

**Files:** Modify `build_health.rs` (`heal`). Test: unit-test the pure planners; the side-effecting steps are manual.

- [ ] **Step 1: Failing test** — the heal planner maps verdicts to actions:
```rust
#[test]
fn heal_plan_for_sccache_and_wsl() {
    assert_eq!(heal_action("sccache.pathological"), HealAction::DisableSccache);
    assert_eq!(heal_action("docker.wsl_wedged"), HealAction::RestartWslDistro);
    assert_eq!(heal_action("toolchain.rustc_shadowed"), HealAction::FlagOnly); // never auto rustup-init
}
```

- [ ] **Step 2: Run it, confirm FAIL** (`heal_action`/`HealAction` undefined).

- [ ] **Step 3: Implement** the planner + executors:
```rust
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HealAction { DisableSccache, RestartWslDistro, FlagOnly }

pub(crate) fn heal_action(diagnosis_id: &str) -> HealAction {
    match diagnosis_id {
        "sccache.pathological" => HealAction::DisableSccache,
        "docker.wsl_wedged" => HealAction::RestartWslDistro,
        _ => HealAction::FlagOnly,
    }
}
```
Executors (side-effecting, behind `auto_heal`):
- `DisableSccache`: `sccache --stop-server`; comment `rustc-wrapper` in `~/.cargo/config.toml` (idempotent, never delete user content); remove the cache dir.
- `RestartWslDistro`: extract the distro name from the docker stderr (default `podman-machine-default`), run `wsl --terminate <distro>`, then restart Docker Desktop — all via the `#[cfg(windows)] quiet_command` helper (no window).
- `FlagOnly`: print the `remediation_command`; change nothing.

- [ ] **Step 4: Run tests, confirm PASS**.

- [ ] **Step 5: Commit** `feat(doctor): --heal for sccache disable + targeted WSL distro restart`.

---

## Task 10: Hidden scheduled task (no flashing window)

**Files:** Modify `scripts/ci/voxcirunnerscale.task.xml`.

- [ ] **Step 1:** add `<Hidden>true</Hidden>` inside `<Settings>` (keep the existing `<ExecutionTimeLimit>PT2M</ExecutionTimeLimit>`):
```xml
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <Hidden>true</Hidden>
    <ExecutionTimeLimit>PT2M</ExecutionTimeLimit>
    ...
  </Settings>
```

- [ ] **Step 2: Re-register** via the existing elevated path (PowerShell `Start-Process schtasks.exe -Verb RunAs -Wait -ArgumentList '/Create','/TN','VoxCIRunnerScale','/XML','scripts/ci/voxcirunnerscale.task.xml','/F'`), then run one tick and confirm no console window flashes.

- [ ] **Step 3: Commit** `chore(ci): run VoxCIRunnerScale hidden (no flashing console)`.

---

## Task 11: Quiet child spawns in the infra paths

**Files:** Modify `crates/vox-cli/src/commands/ci/runner_scale.rs` and any watchdog/autoscaler `Command::new` spawns; reuse the existing `#[cfg(windows)] quiet_command` helper (`CREATE_NO_WINDOW`).

- [ ] **Step 1: Failing test** — a guard test that the infra paths have no bare windowed spawn:
```rust
#[test]
fn no_windowed_spawn_in_runner_scale() {
    let src = include_str!("runner_scale.rs");
    // every Command::new(... docker|gh|vox ...) in this file must go through quiet_command
    assert!(!src.contains("Command::new(\"docker\")"), "use quiet_command(\"docker\") to avoid a console window");
}
```

- [ ] **Step 2: Run it, confirm FAIL** if any bare spawn exists.

- [ ] **Step 3: Replace** each bare `Command::new("docker"|"gh"|"vox")` in the autoscaler path with `quiet_command(...)` (the helper that applies `CREATE_NO_WINDOW` under `#[cfg(windows)]`, plain `Command` elsewhere). Do the same in the `ci-runners-up.vox` `process.run` path if it surfaces a window (the VoxScript builtin should already set `CREATE_NO_WINDOW` per `run_global` precedent — verify).

- [ ] **Step 4: Run tests, confirm PASS**; manually confirm a tick spawns no windows.

- [ ] **Step 5: Commit** `fix(ci): route autoscaler child spawns through quiet_command (no windows)`.

---

## Self-Review notes
- **Spec coverage:** Part 1 (toolchain integrity=T3, Docker/WSL=T4, compile probe=T7, sccache guard=T6) + schema drift (T5, new); Part 2 (Diagnosis=T2); Part 4 (hidden task=T10, quiet spawns=T11); auto-heal matrix=T9; plus the ANSI fix (T1, new). All mapped.
- **New vs spec:** T1 (ANSI/non-TTY) and T5 (schema drift) extend the spec per the latest scheduler output; both are instances of its "surface properly / formatted properly" goal.
- **Type consistency:** `Check`/`Diagnosis`/`Severity` defined in T2 and used unchanged in T3–T9; `with_diagnosis(id, severity, root_cause, remediation_command, auto_healable)` signature identical throughout.
- **No per-build overhead:** every check is in on-demand `vox doctor`; the compile probe is the only multi-second step and runs only on full `vox doctor`.

## Verification (end-to-end)
`vox doctor --json | jq '.[].diagnosis'` shows structured findings; on a healthy machine all checks pass; force a break (`RUSTC_WRAPPER=false vox doctor`) → red compile-probe with `toolchain.compile_failed`; the scheduled tick's redirected output has zero ANSI bytes; one tick spawns no console window.
