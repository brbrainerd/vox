---
title: Build/infra health — loud, LLM-readable, self-healing diagnostics
category: Operations
status: design
date: 2026-06-29
companion: 2026-06-29-sccache-acceleration-design.md (sccache speedup — separate track)
---

# Build/infra health doctor

## Context

A single session's worth of build failures were **invisible until they cost hours**:
`cargo --version` printed `cargo 1.96.0` while every real build aborted (a shim
ping-pong); `rustc --version` printed `cargo 1.96.0` (rustup destroyed); sccache
segfaulted rustc (`STATUS_ACCESS_VIOLATION`) on a corrupt 41 GB cache; the autoscaler's
`docker info` precondition silently failed when the WSL distro wedged
(`docker-desktop-user-distro … Permission denied`); and the autoscaler task flashed a
console window every 2 minutes. None of it surfaced. The existing `vox doctor`
toolchain check runs only `--version` probes, which **lie green** through every one of
these failures.

## Goals

- **Surface loudly**: each build/infra pathology becomes a red `vox doctor` check that
  names the symptom, root cause, and exact fix.
- **LLM-readable**: structured JSON output an agent can read and *immediately* act on —
  no re-diagnosing from scratch.
- **Self-heal what's safe** (`vox doctor --heal`), flag what isn't.
- **No per-build overhead**: detection lives in on-demand `vox doctor`, not the build path.
- **No workflow interruption**: scheduled tasks + child spawns run hidden — zero flashing
  console windows.

## Non-goals

- **sccache acceleration** (making caching actually deliver hits) — its own spec
  (`2026-06-29-sccache-acceleration-design.md`). Here sccache is only *guarded*: detect
  the crash / 0%-hit signature and recommend, don't tune it.
- Per-build guard hooks (rejected: touches every build path).

## Design

### Part 1 — build-health check (`checks_standard/build_health.rs`)

A new check module returning `Vec<Check>`, registered in the standard doctor registry
(`checks_standard/mod.rs`), three layers cheapest-first:

**(a) Toolchain integrity (instant, no compile)** — the probes `--version` misses:
- `rustc --version` MUST start with `rustc` (printed `cargo …` when shadowed).
- `rustup --version` MUST start with `rustup` (ran cargo when destroyed).
- `~/.cargo/bin/cargo` resolves to a real proxy, not a multi-MB shim — checked via file
  size + `VOX_BROKER_DEBUG=1 cargo --version`: if the reported "real cargo" points back
  into a shim dir (`.vox/build-broker/bin` or a shim-sized `~/.cargo/bin/cargo`), that's
  the ping-pong. Remediation names `rustup-init … --no-modify-path --default-toolchain none`.

**(b) Docker / WSL reachability (cross-platform check, platform-aware heal)** —
`docker info` (short timeout). On failure, classify:
- WSL signature (Windows): stderr matches `docker-desktop-user-distro|Permission denied|
  unexpectedly stopped|wslErrorCode` → root_cause "WSL distro wedged", heal =
  `wsl --terminate podman-machine-default` + Docker Desktop restart (targeted — does NOT
  bounce other distros).
- Generic: "Docker daemon unreachable" → Linux heal `systemctl restart docker`, else flag.
This is the autoscaler's first precondition, so a wedged WSL silently starves the fleet —
now it's a red check.

**(c) Real compile probe (keystone, ~seconds, on-demand only)** — compile a trivial
throwaway crate through the *as-configured* toolchain (wrapper included) in a temp dir
with a ~30 s timeout. Shim ping-pong, sccache segfault, and a broken rustc all fail here
while `--version` lies green. Gated behind on-demand doctor, so the seconds don't matter.

**sccache guard (read-only here):** if sccache is the configured wrapper, surface
`sccache --show-stats --stats-format=json` — flag `compilation_failures > 0` (crash) and
hit-rate `< 5%` over a meaningful sample. *Tuning* hits is the other spec; here we only
catch the regression and (on --heal) fall back to disabling, exactly as done by hand.

### Part 2 — LLM-readable surfacing

Extend the check payload (new `Diagnosis` struct surfaced in `--json`) beyond `pass/detail`:

```
{ id, severity, symptom, root_cause, remediation_command, auto_healable, docs_url }
```

So an agent hitting an opaque build failure runs `vox doctor --json` and gets, e.g.,
`{id:"toolchain.rustc_shadowed", root_cause:"rustc resolves to a cargo forwarder",
remediation_command:"rustup-init -y --no-modify-path --default-toolchain none", auto_healable:false}`.
Human-facing text output stays as-is; the structured fields are additive (back-compat:
existing `--json` consumers keep their fields).

### Part 4 — spawn & scheduler hygiene (no flashing windows)

- **Scheduled task**: `scripts/ci/voxcirunnerscale.task.xml` runs hidden —
  `<Settings><Hidden>true</Hidden></Settings>` and run-whether-logged-on so the 2-min tick
  never flashes a console. Re-register via the existing elevated path.
- **Child spawns**: audit every `Command::new` in the autoscaler/watchdog paths
  (`crates/vox-cli/src/commands/ci/runner_scale.rs`, the `ci-runners-up.vox` `process.run`
  docker probe, any `gh`/`docker`/`vox` spawn) and route through the existing
  `#[cfg(windows)] CREATE_NO_WINDOW` `quiet_command` helper. A doctor check counts windowed
  spawns in these paths as a regression guard.

### Auto-heal matrix (`vox doctor --heal`)

| Pathology | Heal action | Auto? |
|---|---|---|
| sccache crashing / ~0% hits | stop server, clear cache, comment `rustc-wrapper` | yes |
| sccache cache > bound | clear cache | yes |
| WSL/Docker wedged | `wsl --terminate podman-machine-default` + Docker restart | yes (targeted) |
| Docker down (Linux) | `systemctl restart docker` | yes |
| rustc/rustup/cargo shim-shadowed | **flag only** — print rustup-init command | no (too invasive) |
| windowed spawn in infra path | **flag only** — code fix | no |

## Components & files

| File | Role |
|---|---|
| `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/build_health.rs` | new — the 3-layer check + sccache guard |
| `…/doctor/checks_standard/mod.rs` | register the check |
| `…/doctor/common.rs` | add the `Diagnosis` struct + `--json` emission |
| `…/doctor/heal*.rs` (existing auto_heal path) | the heal matrix actions |
| `scripts/ci/voxcirunnerscale.task.xml` | `<Hidden>true</Hidden>` + run-whether-logged-on |
| `crates/vox-cli/src/commands/ci/runner_scale.rs` + watchdog spawns | route through `quiet_command` |

## Testing

- Toolchain integrity: unit-test the classifier against captured strings (`"cargo 1.96.0"`
  as rustc output → shadowed; real `"rustc 1.96.0"` → ok).
- WSL classifier: unit-test the exact pasted stderr → WSL-wedged diagnosis.
- Real compile probe: integration test that a deliberately-broken `RUSTC_WRAPPER=false`
  yields a red check (and that a healthy toolchain is green).
- `--json` shape: assert the `Diagnosis` fields are present and machine-parseable.
- Hidden-spawn guard: a test that greps the infra paths for windowed `Command::new` without
  the helper.

## Risks

- **Real compile probe cost**: ~seconds on a cold temp crate. Mitigate: tiny `fn main(){}`,
  cache the temp dir, only run in full (`vox doctor`, not `--quick`).
- **WSL `--terminate` collateral**: targeted to `podman-machine-default` only; if Docker
  Desktop uses a differently-named distro, the heal no-ops and falls back to flag (detect
  the distro name from the error, don't hardcode blindly).
- **Auto-heal editing `~/.cargo/config.toml`**: only ever comments `rustc-wrapper` (never
  deletes user content); idempotent.

## Audit corrections (2026-06-29, codebase-verified)

An adversarial audit revised four design points (the plan carries the exact symbols):

- **Reuse, don't reinvent.** `crates/vox-cli/src/commands/ci/doctor_build_cache.rs::advise(...)`
  already gives sccache *setup* advice; the sccache layer calls it and adds only the new
  runtime-health check (crash + hit-rate from `--show-stats`). `quiet_command` already exists
  (`runner_scale.rs:226`) — Part 4 audits/reuses it, never reimplements `CREATE_NO_WINDOW`.
- **Compile probe is configurable**, not a hard 30 s: `VOX_DOCTOR_COMPILE_TIMEOUT_SECS`
  (default 30) to avoid false positives on slow/minimal VMs; cache the built probe at
  `~/.vox/doctor-probe`. (Supersedes the "cache the temp dir, not `--quick`" risk note — no
  `--quick` flag is introduced.)
- **`Diagnosis` is a versioned, discoverable contract**, not an ad-hoc struct: a registered
  `DiagnosisId` enum (`diagnoses.rs`) with a `schema_version`, surfaced via `vox doctor --json`,
  so agents consume a stable machine contract. It stays in-memory `--json` only — **no new
  `doctor_findings` table**; persistence, if ever needed, extends existing build telemetry
  (`project_check.rs` / `ops_build`).
- **Schema-drift remediation caveat:** "rebuild vox" only helps if the working tree actually
  contains the newer migration; if the DB was bumped by a *different* branch's binary, the
  honest diagnosis is "this DB is from a newer vox than your source" — the check states both.
- **Freshness reality:** the `runner-*` freshness exemption was reverted (`run_body.rs:56` is
  unconditional `enforce_for_ci`), so autoscaler-vs-freshness is unresolved and out of scope
  here; this check only *surfaces* the related schema-drift symptom.
