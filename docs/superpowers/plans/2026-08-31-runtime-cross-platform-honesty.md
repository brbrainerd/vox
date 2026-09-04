# Runtime and Cross-Platform Honesty Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Vox’s “runs on many systems” claims are CI-gated or explicitly refused: required OS smoke, wasm-vs-container capability manifests, one skill runner, and honest docs/diagnostics for mobile, cloud, CUDA, durability, and deploy.

**Architecture:** Live `cross-platform-check.yml` already has `pull_request` + `merge_group` + schedule and per-PR `cargo check --workspace` on Win/mac/Linux. This plan **does not add `pull_request`**. It (1) syncs stale CI docs, (2) documents admin required-check for the existing job, (3) **adds** `cargo test -p vox-compiler --lib` on merge_group/schedule, one hosted OS, (4) **adds** `vox run --interp` golden on merge_group, one OS. Capability manifests, one skill runner (CLI ARS stub vs MCP `SandboxedSkillRunner`), honest docs/diagnostics. Coverage: [`2026-08-31-platform-parity-id-coverage.md`](../specs/2026-08-31-platform-parity-id-coverage.md).

## Audit corrections (spec §9)

- Test that only asserts `pull_request:` **passes today** and gates nothing — assert **depth policy** instead (`cargo check --workspace` on PR; Win/mac nextest off PR).
- `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.github/workflows/cross-platform-check.yml"))` from `crates/vox-cli`.
- Never make a merge_group-only job a **required** check.
- R11: determinism lint already default-on — fixture: `time.now()` inside a **workflow**.
- R12: `deploy --dry-run` exists — fixture exit 0/4.
- R10: `--fix-cuda-path` exists; add `VOX_REQUIRE_CUDA` + env registry.
- R07: CLI `vox skill run` → ARS echo stub; MCP already `SandboxedSkillRunner`.

**Tech Stack:** GitHub Actions, `vox-cli` doctor, `vox-skills` sandbox, `vox-container`, contracts for capabilities.

**Spec:** [`docs/superpowers/specs/2026-08-31-platform-parity-design.md`](../specs/2026-08-31-platform-parity-design.md) R01–R12. Background: [`docs/src/architecture/cross-platform-guarantees-audit-and-enforcement-2026-06-15.md`](../../src/architecture/cross-platform-guarantees-audit-and-enforcement-2026-06-15.md) (A1 was full matrix — this plan ships **slim smoke** plus a required check, because anti-stacking previously moved the heavy workflow off PRs).

**Closes:** R01–R12.

## Global Constraints

Inherit spec §6. Runner labels: self-hosted Linux for heavy jobs; GitHub-hosted Win/macOS only with a `docs/src/ci/github-hosted-exceptions.md` row (already present for `cross-platform-check.yml`). Do not add unregistered `ubuntu-latest`. Capability files are contracts, not ad-hoc JSON in `target/`.

---

## File map

| File | Role |
|---|---|
| Modify: `.github/workflows/cross-platform-check.yml` | slim required job |
| Modify: `docs/src/ci/github-hosted-exceptions.md` | note required-smoke |
| Create: `contracts/runtime/capabilities.v1.yaml` | capability vocab |
| Modify: `crates/vox-cli` doctor | mismatch errors |
| Modify: `crates/vox-skills/src/sandbox/runner.rs` | CLI path |
| Modify: `crates/vox-cli` skill run | call runner |
| Modify: packaging / doctor CUDA | fail-loud |
| Modify: determinism lint default | `vox check` |

---

### Task 1: Required check + honest CI docs (R01) — do not add `pull_request`

**Files:** `docs/src/ci/github-hosted-exceptions.md`, `docs/src/ci/runner-autoscaling.md`, `docs/src/ci/runner-contract.md`; optional YAML only for merge_group-gated `vox-compiler --lib`; `crates/vox-cli/tests/cross_platform_workflow.rs`.

**Interfaces:**
- Consumes: existing workflow (already `on.pull_request`)
- Produces: docs that match YAML; contract test for **depth**; admin note to require `Cross-Platform (Win/macOS/Ubuntu)` / `cross-check`

- [ ] **Step 1: Failing check** that encodes depth policy, not mere trigger presence:

```rust
#[test]
fn cross_platform_pr_is_check_not_full_nextest() {
    let y = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.github/workflows/cross-platform-check.yml"
    ));
    assert!(y.contains("pull_request"), "already true — keep it");
    assert!(y.contains("cargo check --workspace"), "PR must stay cheap");
    // Win/mac nextest must not run on pull_request — assert the `if:` gating nextest
    assert!(
        y.contains("merge_group") && y.contains("schedule"),
        "deep tests stay amortized"
    );
}
```

- [ ] **Step 2:** If this **passes on HEAD**, the software work is docs + required-check note. Patch exceptions footnote (“Win/mac only on merge_group”) which is **false**.

- [ ] **Step 3:** add job step `cargo test -p vox-compiler --lib` with `if: github.event_name == 'merge_group' || github.event_name == 'schedule'` on **one** hosted OS, timeout 20m. Do **not** run it on every PR. This is **required** for R01 v1 (not optional). Test YAML with `include_str!` that the step exists.

- [ ] **Step 4:** `vox ci runner-policy-check` still green.

- [ ] **Step 5:** commit `docs: sync cross-platform-check PR depth with live YAML; note required-check admin step`

---

### Task 1b: merge_group `vox run --interp` golden (R01 remaining)

Original fix included interp + wasm run. v1: **one** OS on merge_group/schedule runs `vox run --interp` on a golden `.vox` (pick `examples/golden/` file that is interp-safe — `rg --interp examples`). Wasm+subprocess stays doctor (Task 2). GUI sidecar is residual (anti-stacking).

**Files:** `.github/workflows/cross-platform-check.yml`; `crates/vox-cli/tests/cross_platform_workflow.rs`.

- [ ] **Step 1:** `include_str!` YAML contains `vox run --interp` (or `cargo run -p vox-cli -- run --interp`) gated `merge_group`/`schedule`.
- [ ] **Step 2:** FAIL if missing.
- [ ] **Step 3:** add the step; timeout 5m; one OS.
- [ ] **Step 4:** PASS the include test. Do **not** add `pull_request`.
- [ ] **Step 5:** commit `ci: merge_group vox run --interp golden for R01`

---

### Task 2: Capability manifest (R02)

**Files:** `contracts/runtime/capabilities.v1.yaml`; doctor; `vox run --isolation wasm` preflight.

```yaml
# capabilities.v1.yaml
x-vox-version: "1.0.0"
capabilities: [pure, fs, subprocess, gpu, net]
```

Skill/program header comment or `Plugin.toml` key `capabilities = ["subprocess"]`.

- [ ] **Step 1:**

```rust
#[test]
fn wasm_plus_subprocess_is_doctor_error() {
    let report = check_isolation(Isolation::Wasm, &[Cap::Subprocess]);
    assert!(report.is_err());
    assert!(report.unwrap_err().contains("container"));
}

#[test]
fn wasm_plus_pure_ok() {
    assert!(check_isolation(Isolation::Wasm, &[Cap::Pure]).is_ok());
}
```

Put `check_isolation` in `crates/vox-cli` doctor module (`rg "isolation wasm" crates/vox-cli/src`).

- [ ] **Step 2:** FAIL. **Step 3:** implement. **Step 4:** PASS. **Step 5:** commit `feat: refuse wasm isolation when subprocess or gpu is declared`

---

### Task 3: One skill runner (R07)

**Files:** `rg "execute_skill" crates` — CLI path currently stubs. Route through `SandboxedSkillRunner::run`.

- [ ] **Step 1:**

```rust
#[test]
fn cli_skill_run_uses_sandboxed_runner() {
    // source contains SandboxedSkillRunner::run on the CLI path
    let src = include_str!("../src/commands/skill.rs"); // fix path via rg
    assert!(src.contains("SandboxedSkillRunner") || src.contains("sandboxed"));
}
```

A string-include test is brittle. Prefer: invoke the CLI function with a fake runner trait.

```rust
#[test]
fn cli_skill_run_calls_sandbox() {
    let mut fake = FakeRunner::default();
    run_skill_with(&mut fake, "demo").unwrap();
    assert!(fake.ran);
}
```

If injecting a trait is too large, add `debug_assert` log — **no**, use a `#[cfg(test)]` hook `static RAN: AtomicBool` set inside `SandboxedSkillRunner::run` and call CLI run on a missing skill that still enters `run`. Weak but acceptable: integration test that the ARS stub echo **is gone** (`assert!(!src.contains("stub echo"))`).

- [ ] **Step 2–5:** implement + commit `fix: CLI skill run uses the same sandbox as MCP`

---

### Task 4a: Sandbox status + `VOX_REQUIRE_SANDBOX` (R03)

**Files:** doctor; `contracts/config/env-vars.v1.yaml` `VOX_REQUIRE_SANDBOX`.

- [ ] **Step 1:** `sandbox_status()` returns `landlock` / `job-object` / `warning-only` by `cfg(target_os)`. `VOX_REQUIRE_SANDBOX=1` → doctor **nonzero** on `warning-only` (macOS today). Unit tests on existing Landlock/job-object modules still compile.
- [ ] **Step 2–5:** commit `feat: sandbox_status and VOX_REQUIRE_SANDBOX fail-loud`. Seatbelt implementation is residual.

### Task 4b: Drop mobile as v1 claim (R04)

Chosen or-gate: **drop**, do not ship a fake RN golden.

- [ ] **Step 1:** `doctor_mobile_target_refuses` — `vox` / doctor `mobile-emit-incomplete` on `--target mobile`; docs “not a v1 target”.
- [ ] **Step 2–5:** commit `fix: refuse --target mobile until RN emit exists`

### Task 4c: Stop marketing cloud as agent runtime (R05)

Chosen: **stop marketing**. Product arm is Track 6 G04 LAN daemon, not Fly VMs.

- [ ] **Step 1:** packaging / `canonical-runtime-names.md` states cloud = LLM providers, not remote agent VMs. `vox-doc-pipeline --lint-only` on the edited file.
- [ ] **Step 2–5:** commit `docs: cloud means LLM providers not agent VMs`

### Task 4d: Document crates.io deferred (R06)

Chosen: **document**. No flip `publish.enabled`.

- [ ] **Step 1:** YAML comment + `docs/src/reference` sentence. Test: `publish.enabled` still false (`rg` in contract test if one exists).
- [ ] **Step 2–5:** commit `docs: crates.io publish is deferred`

---

### Task 5a: Windows unsigned doctor (R08)

- [ ] **Step 1:** `rg Authenticode` — if missing, release-job **comment** + `vox doctor` `windows-unsigned` **warn** (not fail). Test: doctor code path exists.
- [ ] **Step 2–5:** commit `feat: doctor warns on unsigned Windows exe`. Paid cert in CI secrets is residual (admin).

### Task 5b: Triple matrix docs (R09)

- [ ] **Step 1:** table in `docs/src/architecture/vox-application-packaging-ssot-2026.md`: `linux-x64`, `win-x64`, `darwin-arm64`. `vox build --help` lists those triples. Golden **compile** not full nextest.
- [ ] **Step 2–5:** commit `docs: document vox build triples`

### Task 5c: CUDA fail-loud (R10)

- [ ] **Step 1:** `doctor_require_cuda_fails_without_nvcc` — `VOX_REQUIRE_CUDA=1` (register env) and no nvcc → doctor nonzero.
- [ ] **Step 2–5:** commit `feat: VOX_REQUIRE_CUDA fails doctor without nvcc`

### Task 5d: Determinism lint fixture + GUI string (R11)

- [ ] **Step 1:** `workflow_time_now_fails_check` — `workflow` + `time.now()` fails `vox check`. If already green, keep the test. GUI “cannot replay” string is Track 6 Task 27.
- [ ] **Step 2–5:** commit `test: workflow time.now fails determinism lint`

### Task 5e: Deploy dry-run fixture + CI wrap (R12)

- [ ] **Step 1:** `deploy_dry_run_ok_on_fixture` exit 0; missing → 4. CI job **or** `vox ci` wrapping dry-run on `examples/` compose.
- [ ] **Step 2–5:** commit `test: vox deploy --dry-run fixture contract`

---

## Track 5 gate

HARD: `cargo test -p vox-cli cross_platform_pr_is_check_not_full_nextest wasm_plus_subprocess_is_doctor_error doctor_require_cuda_fails_without_nvcc`

HARD: YAML include tests for merge_group `--interp` (Task 1b) and `vox-compiler --lib` (Task 1 Step 3)

HARD: `vox ci runner-policy-check` (non-strict unless the YAML now violates policy)

SOFT: human adds the new GitHub required check name after the first green PR run.
