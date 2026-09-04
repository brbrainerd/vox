# macOS Compatibility Audit & Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close every macOS defect surfaced during the 2026-09-04 MacBook bring-up, and add automated gates so macOS regressions are caught by CI rather than by the next person who clones on a Mac.

**Architecture:** Three layers, in dependency order. (1) Fix the remaining concrete runtime defects. (2) Convert the one-off portability findings into enforced `vox arch-check` rules, so the class of bug is gated rather than re-discovered. (3) Add a real macOS end-to-end lane that cold-installs and launches the product, because every defect found on 2026-09-04 lived in a path that `cargo check --workspace` never exercises.

**Tech Stack:** Rust 1.96.0 (pinned via `rust-toolchain.toml`), clap, Tauri 2, `vox arch-check` (`crates/vox-arch-check`), GitHub Actions, `act` + colima for local runs, Graphify (`vox graph`).

**Spec:** No separate spec document. This plan implements the findings recorded in `~/dev/memory/vox-macos-setup-blockers.md`, `~/dev/memory/vox-single-installer-gaps.md`, and `~/dev/memory/vox-graphify-code-intelligence.md`, all of which were reproduced on macOS 26.5 / aarch64 on 2026-09-04. PR vox-foundation/vox#477 already landed the first wave; this plan covers what remains.

## Global Constraints

- **Toolchain:** Rust `1.96.0` exactly, from `rust-toolchain.toml`. Do not bump it in this plan.
- **Target triples that are published:** `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `x86_64-apple-darwin`, `aarch64-apple-darwin`. `aarch64-unknown-linux-gnu` and `aarch64-pc-windows-msvc` are **not** published — never emit a download URL for them.
- **No new runtime dependencies.** Portability checks use `std` only. `crates/vox-arch-check` already owns regex-based scanning; extend it rather than adding a crate.
- **Installer parity:** `scripts/install.sh` must stay byte-identical to `docs-astro/public/voxup`, and `scripts/install.ps1` byte-identical to `docs-astro/public/voxup.ps1`. The `documented_install_urls_are_served` test enforces this. Any edit to one requires `cp` to the other in the same commit.
- **Docs frontmatter:** any new `.md` under `docs/src/` requires the YAML frontmatter block described in `AGENTS.md` §Authored Markdown Frontmatter. Files under `docs/superpowers/` are exempt.
- **Graphify:** run `vox graph refresh --auto` once per clone before structural queries; an unbuilt graph returns empty results rather than erroring.
- **Commit discipline:** imperative subject under 72 chars; body explains *why*. Branch before committing; never push to `main`.

---

### Task 1: Repair the rotted `populi` feature and unblock mesh

`vox populi status` fails with `unrecognized subcommand 'populi'`. Two earlier readings were both wrong and must not be carried forward:

1. **"Dead CLI surface — the subcommand doesn't exist."** Wrong. `crates/vox-ml-cli/src/main.rs:29` defines a `Populi` variant behind `#[cfg(feature = "populi")]`, and `populi` is a declared feature (`populi = ["dep:vox-populi", "vox-populi/transport", "dep:blake3"]`). It is simply not in `default = ["mens-base"]`.
2. **"Just install with `--features populi`."** Also wrong, and this is the finding that matters: **that build does not compile.** Verified 2026-09-04:

```
error[E0063]: missing fields `accepts_inference_workloads`,
  `accepts_sensitive_training_data`, `accepts_training_workloads`
  and 3 other fields in initializer of `WorkerDonationPolicy`
  --> crates/vox-ml-cli/src/commands/populi_lifecycle.rs:70:35
```

`vox_mesh_types::WorkerDonationPolicy` gained six fields and its only feature-gated consumer was never updated. **This is the same class of defect as the `vox-gamify` feature gate fixed in PR #477**: `cargo check --workspace` does not enable non-default features, so `populi` has been rotting with nothing to catch it. Mesh has been unbuildable, not merely unshipped.

Step 4 fixes the rot; Step 6 stops it recurring, and is the more important half.

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/populi_lifecycle.rs:70`
- Modify: `crates/vox-cli/src/main.rs:110-118` (the delegation failure message)
- Modify: `.github/workflows/cross-platform-check.yml` (the `standalone-installables` job from PR #477)
- Modify: `CONTRIBUTING.md` (the subsystem table added in #477)
- Test: `crates/vox-cli/tests/ml_delegation_hint.rs` (create)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: the exact remediation string `cargo install --path crates/vox-ml-cli --features populi`, reused verbatim by Task 7's workflow and Task 8's doctor check.

- [ ] **Step 1: Reproduce the compile failure**

```bash
cargo build -p vox-ml-cli --features populi 2>&1 | tail -20
```

Expected: the `E0063` above. **Do not pipe this through `tail` alone when checking success** — a pipeline's exit status is the last command's, which masked this exact failure twice during the 2026-09-04 session. Use `cargo build ... > /tmp/out.log 2>&1; echo $?`.

- [ ] **Step 2: Write the failing test**

Create `crates/vox-cli/tests/ml_delegation_hint.rs`:

```rust
//! The hint `vox` prints when `vox-ml-cli` is missing must name a command that
//! actually produces the delegated subcommand. `populi` is behind a non-default
//! feature, so a bare `cargo install --path crates/vox-ml-cli` yields a binary
//! without it — and the user retries the same failing command.

#[test]
fn ml_cli_install_hint_enables_the_populi_feature() {
    let src = include_str!("../src/main.rs");
    let hint_line = src
        .lines()
        .find(|l| l.contains("cargo install --path crates/vox-ml-cli"))
        .expect("delegation failure message must name the install command");
    assert!(
        hint_line.contains("--features populi"),
        "install hint must enable the `populi` feature, got: {hint_line}"
    );
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-cli --test ml_delegation_hint`
Expected: FAIL — `install hint must enable the "populi" feature`.

- [ ] **Step 4: Fix the rotted initializer**

`WorkerDonationPolicy` has **no `Default` impl**, so every field must be named. Add the six missing fields to the initializer at `crates/vox-ml-cli/src/commands/populi_lifecycle.rs:70`, after the existing `slots: …` entry:

```rust
                // Conservative defaults: donate compute, never sensitive data, and
                // advertise no accelerator tier until the operator opts in.
                accepts_inference_workloads: true,
                accepts_training_workloads: false,
                accepts_sensitive_training_data: false,
                cuda_tier: 0,
                metal_tier: 0,
                vram_min_gb: 0,
```

Read `crates/vox-mesh-types/src/donation_policy.rs` before writing these — if the field set has drifted again, match the struct, not this list. `metal_tier` is the Apple-GPU rung; leaving it `0` advertises no Metal capability, which is the safe default until Task 7's lane can prove the Metal plugin loads.

- [ ] **Step 5: Verify the feature compiles and mesh answers**

```bash
cargo build -p vox-ml-cli --features populi > /tmp/populi.log 2>&1; echo "EXIT=$?"
cargo install --path crates/vox-ml-cli --features populi --locked --debug
vox-ml-cli --help | grep -q populi && echo "populi subcommand present"
vox populi status
cargo test -p vox-cli --test ml_delegation_hint
```

Expected: `EXIT=0`; `populi subcommand present`; `vox populi status` returns real output rather than `unrecognized subcommand`; the hint test passes.

- [ ] **Step 6: Stop the feature rotting again**

In `.github/workflows/cross-platform-check.yml`, extend the `standalone-installables` job's per-crate check step so the non-default feature is actually compiled:

```yaml
      - name: cargo check (standalone, per-crate)
        run: |
          cargo check -p vox-ml-cli
          cargo check -p vox-ml-cli --features populi
          cargo check -p vox-gamify
          cargo check -p vox-orchestrator-d
```

This is the step that matters. Without it the next struct change re-breaks mesh silently, exactly as this one did.

- [ ] **Step 7: Update the CONTRIBUTING table**

In `CONTRIBUTING.md`, change the ML/mesh CLI row's install command to
`cargo install --path crates/vox-ml-cli --features populi` and append to that row's
"Needed for" cell: `` (`populi` is not a default feature) ``.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-ml-cli/src/commands/populi_lifecycle.rs \
        crates/vox-cli/src/main.rs \
        crates/vox-cli/tests/ml_delegation_hint.rs \
        .github/workflows/cross-platform-check.yml \
        CONTRIBUTING.md
git commit -m "Repair the populi feature and compile it in CI"
```

- [ ] **Step 9: Correct the public record**

Edit PR #477's description: replace the "Deliberately not fixed — `vox populi` is dead CLI surface" bullet with the rotted-feature explanation, and note that mesh was unbuildable rather than merely unshipped. Update `~/dev/memory/vox-macos-setup-blockers.md` the same way. **Do not leave either incorrect claim standing** — "the subcommand doesn't exist" sends the next reader hunting for missing code, and "just add `--features populi`" sends them into a compile error.

---

### Task 2: Platform-correct the CUDA plugin artifact name

`crates/vox-ml-cli/src/commands/mens/plugin_heal.rs:34` uses `#[cfg(not(windows))]` to pick `libvox_plugin_mens_candle_cuda.so`. On macOS a `cdylib` builds as `.dylib`, so the non-Windows branch names a file that can never exist there. CUDA is irrelevant on macOS, so this is latent rather than breaking — but the `cfg(not(windows)) ⇒ .so` shape is the exact pattern Task 5 gates, and leaving a live instance in-tree makes that rule unlandable.

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/mens/plugin_heal.rs:30-36`
- Test: same file, `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-ml-cli/src/commands/mens/plugin_heal.rs`:

```rust
#[cfg(test)]
mod artifact_tests {
    use super::ARTIFACT;

    #[test]
    fn artifact_uses_the_platform_dylib_suffix() {
        let expected = if cfg!(windows) {
            ".dll"
        } else if cfg!(target_os = "macos") {
            ".dylib"
        } else {
            ".so"
        };
        assert!(
            ARTIFACT.ends_with(expected),
            "ARTIFACT `{ARTIFACT}` must end with `{expected}` on this target"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-ml-cli --lib artifact_uses_the_platform_dylib_suffix`
Expected: FAIL on macOS — `ARTIFACT "libvox_plugin_mens_candle_cuda.so" must end with ".dylib"`.

- [ ] **Step 3: Add the macOS branch**

Replace the two-arm `cfg` block with three arms:

```rust
#[cfg(windows)]
const ARTIFACT: &str = "vox_plugin_mens_candle_cuda.dll";
// vox-arch-check: allow dynlib-ext
#[cfg(target_os = "macos")]
const ARTIFACT: &str = "libvox_plugin_mens_candle_cuda.dylib";
// vox-arch-check: allow dynlib-ext
#[cfg(all(not(windows), not(target_os = "macos")))]
const ARTIFACT: &str = "libvox_plugin_mens_candle_cuda.so";
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-ml-cli --lib artifact_uses_the_platform_dylib_suffix`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-ml-cli/src/commands/mens/plugin_heal.rs
git commit -m "Use the macOS dylib suffix for the CUDA plugin artifact"
```

---

### Task 3: Decide the ACI shell backend for POSIX hosts

`crates/vox-orchestrator-mcp/src/aci/envelope.rs:47` returns `"powershell"` for `vox_run_shell` on every platform, including when no backend is requested. On macOS that labels the envelope with a shell that is usually absent.

This was left unfixed in PR #477 on purpose: `contracts/aci/agent-computer-interface.v1.yaml` defines exactly three values — `contract_first`, `powershell`, `nushell`. There is no POSIX adapter, so a "correct" macOS default cannot be invented in code.

**This task is blocked on an owner decision.** Do not guess. Open a discussion with the ACI owner and pick one:

- **(A) Default to `contract_first` when no backend is requested.** The SSOT already describes it as "Shell-agnostic; backend binding supplied separately". Smallest change, no contract edit. Risk: changes the value emitted for existing callers.
- **(B) Add a `posix` adapter** to the YAML SSOT, the two JSON schemas, and the normalizer. Most correct, largest blast radius.
- **(C) Keep `powershell`, document it as a telemetry label rather than an execution target.** Zero code change; requires a comment at the call site so the next reader does not re-file this.

**Files (option A — the recommended default if the owner has no preference):**
- Modify: `crates/vox-orchestrator-mcp/src/aci/envelope.rs:42-58`
- Test: same file, existing `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Get the decision in writing**

Comment on PR #477 tagging the ACI owner with the three options above. Record the answer in the commit body. **If no answer within the working session, stop here and move to Task 4** — this task is not on the critical path.

- [ ] **Step 2 (option A only): Write the failing test**

Add to the existing `mod tests` in `envelope.rs`:

```rust
#[test]
fn aci_shell_backend_defaults_to_contract_first_without_args() {
    let v = super::aci_shell_backend_for_tool("vox_run_shell", None);
    assert_eq!(v, serde_json::Value::String("contract_first".into()));
}
```

- [ ] **Step 3 (option A only): Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp aci_shell_backend_defaults_to_contract_first_without_args`
Expected: FAIL — left `"powershell"`, right `"contract_first"`.

- [ ] **Step 4 (option A only): Change the no-args default**

In `aci_shell_backend_for_tool`, replace the `let Some(args) = tool_args else` arm:

```rust
    let Some(args) = tool_args else {
        // No backend requested: the ACI SSOT's shell-agnostic value. Defaulting to
        // `powershell` mislabeled every POSIX host, where pwsh is usually absent.
        return Value::String("contract_first".into());
    };
```

Leave the explicit-value normalizer (`"nu" | "nushell"`, `"pwsh" | "powershell" | …`) untouched — Task 3 changes only the unspecified case.

- [ ] **Step 5 (option A only): Run the full envelope suite**

Run: `cargo test -p vox-orchestrator-mcp aci`
Expected: PASS. The existing `aci_shell_backend_for_run_shell_nushell_when_requested` must still pass; if a test asserting `"powershell"` for the no-args case fails, that test encodes the old default — update it and say so in the commit body.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/aci/envelope.rs
git commit -m "Default the ACI shell backend to contract_first when unspecified"
```

---

### Task 4: Make `Workspace Registration` honest

`vox doctor`'s `Workspace Registration` check reads `project.vox-workspace.path` from the DB, but **nothing in the tree ever writes that key** (`grep -rn '"vox-workspace"' crates/ --include='*.rs'` returns only the doctor read site). The check therefore fails on every install, forever. PR #477 corrected only the misleading remediation text.

A permanently-failing check trains people to ignore doctor output, which is what let the genuinely broken checks hide.

**Files:**
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs:305-325`
- Test: `crates/vox-cli/tests/doctor_workspace_registration.rs` (create)

**Interfaces:**
- Consumes: nothing.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Confirm nothing writes the key**

```bash
grep -rn '"vox-workspace"' crates/ --include='*.rs'
```

Expected: only `checks_standard/tail.rs`. If a writer exists, this task is void — wire the check to it and skip to Task 5.

- [ ] **Step 2: Write the failing test**

Create `crates/vox-cli/tests/doctor_workspace_registration.rs`:

```rust
//! A doctor check that no code path can ever satisfy is noise, not signal.
//! Nothing writes `project.vox-workspace.path`, so this check must be
//! informational until a registration command exists.

#[test]
fn workspace_registration_is_not_a_hard_failure() {
    let src = include_str!(
        "../src/commands/diagnostics/doctor/checks_standard/tail.rs"
    );
    let idx = src
        .find("name: \"Workspace Registration\"")
        .expect("the Workspace Registration check must exist");
    let window = &src[idx..idx + 200];
    assert!(
        !window.contains("pass: reg_pass"),
        "Workspace Registration must not gate on a key nothing writes; \
         mark it informational until a registration command exists"
    );
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-cli --test doctor_workspace_registration`
Expected: FAIL — `must not gate on a key nothing writes`.

- [ ] **Step 4: Make the check informational**

In `tail.rs`, replace the `checks.push` for `Workspace Registration`:

```rust
    checks.push(Check {
        name: "Workspace Registration".to_string(),
        // Informational: no CLI command writes `project.vox-workspace.path` yet, so
        // a hard failure here is permanent noise. Flip back to `reg_pass` once a
        // registration command exists.
        pass: true,
        detail: if reg_pass {
            reg_detail
        } else {
            "not registered (optional — no registration command exists yet)".to_string()
        },
    });
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-cli --test doctor_workspace_registration`
Expected: PASS.

- [ ] **Step 6: Verify doctor's failure count drops by one**

```bash
cargo build -p vox-cli
./target/debug/vox doctor 2>&1 | grep -cE '^  ✗'
```

Expected: one fewer than before the change. Record both numbers in the commit body.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs \
        crates/vox-cli/tests/doctor_workspace_registration.rs
git commit -m "Make Workspace Registration informational until a writer exists"
```

---

### Task 5: Gate the portability patterns that actually break macOS

`graphify-out/OS_COMPATIBILITY.md` lists 306 un-gated findings, but it is a **report**, not a gate — nothing fails when a new one lands, and it is generated by `scripts/coverage-graph/os_compat.py` (Python), separate from Graphify. `crates/vox-arch-check/src/forbidden_patterns.rs` already has the right mechanism: `no-hardcoded-dynlib-ext`, with an `allow_annotation` escape hatch.

Do **not** try to gate all 306 — most are test fixtures, doc comments, and file-extension lists. Gate the two categories whose un-gated hits are genuinely load-bearing on macOS.

**Files:**
- Modify: `crates/vox-arch-check/src/forbidden_patterns.rs` (add two rules beside `dynlib_ext_rule`)
- Test: same file's `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: the existing `ForbiddenPatternRule` struct — fields `name: String`, `exempt_tests: bool`, `pattern: String`, `file_glob: String`, `exempt_files: Vec<String>`, `allow_annotation: Option<String>`, `reason: String`.
- Produces: rule names `no-literal-home-tilde` and `no-hardcoded-tmp-path`, referenced by Task 6's CI step.

- [ ] **Step 1: Establish the true baseline**

```bash
rg -n '"~/' crates --glob '*.rs' | grep -v '/tests/' | grep -v '^.*//' | wc -l
rg -n '"/tmp/' crates --glob '*.rs' | grep -v '/tests/' | wc -l
```

Record both counts. Every remaining hit must either be fixed or carry an allow annotation before Step 5 can pass — if a count is large (>15), split this task per rule and land them separately.

- [ ] **Step 2: Write the failing tests**

Add to `forbidden_patterns.rs`'s `mod tests`:

```rust
fn home_tilde_rule() -> ForbiddenPatternRule {
    ForbiddenPatternRule {
        name: "no-literal-home-tilde".into(),
        exempt_tests: true,
        // A string literal starting with `~/`. Rust never expands tilde, so this
        // is a path that resolves to a literal "~" directory on every OS.
        pattern: r#""~/"#.into(),
        file_glob: "crates/**/*.rs".into(),
        exempt_files: vec![],
        allow_annotation: Some("// vox-arch-check: allow home-tilde".into()),
        reason: "Rust does not expand `~`; use the `dirs`/`home` crate for the home directory."
            .into(),
    }
}

fn tmp_path_rule() -> ForbiddenPatternRule {
    ForbiddenPatternRule {
        name: "no-hardcoded-tmp-path".into(),
        exempt_tests: true,
        pattern: r#""/tmp/"#.into(),
        file_glob: "crates/**/*.rs".into(),
        exempt_files: vec![],
        allow_annotation: Some("// vox-arch-check: allow tmp-path".into()),
        reason: "`/tmp` does not exist on Windows and is not the macOS temp dir; use std::env::temp_dir()."
            .into(),
    }
}

#[test]
fn home_tilde_rule_flags_literal_tilde_paths() {
    let dir = tempfile::tempdir().unwrap();
    write_fixture(&dir, "crates/x/src/a.rs", r#"let p = "~/.vox/corpus.jsonl";"#);
    let hits = scan_rule(
        dir.path(),
        &home_tilde_rule(),
        &crate::built_in_walk_prune_names(),
    )
    .unwrap();
    assert_eq!(hits.len(), 1);
}

#[test]
fn home_tilde_rule_respects_allow_annotation() {
    let dir = tempfile::tempdir().unwrap();
    write_fixture(
        &dir,
        "crates/x/src/a.rs",
        "// vox-arch-check: allow home-tilde\nlet p = \"~/.vox\";",
    );
    let hits = scan_rule(
        dir.path(),
        &home_tilde_rule(),
        &crate::built_in_walk_prune_names(),
    )
    .unwrap();
    assert!(hits.is_empty());
}

#[test]
fn tmp_path_rule_flags_hardcoded_tmp() {
    let dir = tempfile::tempdir().unwrap();
    write_fixture(&dir, "crates/x/src/a.rs", r#"let p = "/tmp/vox.sock";"#);
    let hits = scan_rule(
        dir.path(),
        &tmp_path_rule(),
        &crate::built_in_walk_prune_names(),
    )
    .unwrap();
    assert_eq!(hits.len(), 1);
}
```

If `scan_rule` / `write_fixture` are named differently, copy the exact helper names used by the neighbouring `exempt_tests_skips_tests_dir_and_cfg_test_blocks` test in the same module.

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p vox-arch-check forbidden_patterns`
Expected: FAIL — `home_tilde_rule` / `tmp_path_rule` not found.

- [ ] **Step 4: Register the rules in the production rule set**

Add both constructors next to `dynlib_ext_rule` in the non-test module and push them into the same `Vec<ForbiddenPatternRule>` the binary consumes. Follow whatever registration function `no-hardcoded-dynlib-ext` uses — do not invent a parallel list.

- [ ] **Step 5: Run tests and the real scan**

```bash
cargo test -p vox-arch-check forbidden_patterns
cargo run -p vox-cli -- diag arch-check
```

Expected: tests PASS. The scan will report real hits — for each, either replace with `dirs::home_dir()` / `std::env::temp_dir()`, or add the allow annotation with a one-line reason. `crates/vox-ml-cli/src/commands/mens/pipeline.rs` and `crates/vox-config/src/operator_registry.rs` are known hits from the 2026-09-04 report; re-verify their line numbers, which have drifted.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-arch-check/src/forbidden_patterns.rs
git commit -m "Gate literal ~ and /tmp paths in vox arch-check"
```

---

### Task 6: Regenerate the portability report on macOS

`graphify-out/OS_COMPATIBILITY.md` is committed but stale — its line numbers no longer match the tree (verified 2026-09-04: `vox-ml-cli/src/commands/mens/pipeline.rs:187` and `vox-config/src/operator_registry.rs:831` both point at unrelated code). It was last generated on a non-macOS host.

**Files:**
- Modify: `graphify-out/OS_COMPATIBILITY.md` (regenerated output)
- Modify: `.github/workflows/os-compat-report.yml`

**Interfaces:**
- Consumes: rule names from Task 5.
- Produces: a refreshed report other tasks cite for line numbers.

- [ ] **Step 1: Regenerate on this Mac**

```bash
uv run --with pyyaml python scripts/coverage-graph/os_compat.py --repo-root . --out graphify-out/OS_COMPATIBILITY.md
git diff --stat graphify-out/OS_COMPATIBILITY.md
```

`uv` is the house Python runner (`~/dev/AGENTS.md`); do not `pip install` into the Homebrew Python, which is PEP 668 externally-managed.

- [ ] **Step 2: Confirm the counts moved**

Compare the new "Total un-gated portability findings" against the committed 306. A large drop means Task 5's fixes landed; a rise means new findings — triage them before committing.

- [ ] **Step 3: Add a macOS leg to the report workflow**

In `.github/workflows/os-compat-report.yml`, change the job's `runs-on` to a matrix over `ubuntu-latest` and `macos-latest`, so the scan runs where the `.dylib`/`.so` asymmetry is actually observable. Keep the existing `python scripts/coverage-graph/os_compat.py --repo-root . --out graphify-out/OS_COMPATIBILITY.md` invocation unchanged.

- [ ] **Step 4: Validate the workflow parses**

```bash
uv run --with pyyaml python -c "import yaml; d=yaml.safe_load(open('.github/workflows/os-compat-report.yml')); print(list(d['jobs'].keys()))"
act --list --workflows .github/workflows/os-compat-report.yml
```

Expected: both succeed; `act --list` shows the job.

- [ ] **Step 5: Commit**

```bash
git add graphify-out/OS_COMPATIBILITY.md .github/workflows/os-compat-report.yml
git commit -m "Regenerate portability report on macOS and add a macOS CI leg"
```

---

### Task 7: Cold-install end-to-end lane on macOS

Every defect found on 2026-09-04 — the `vox-gamify` feature gate, the 16-bit icons, the pristine-`~/.zshrc` PATH gap — lived in a path that `cargo check --workspace` never touches. PR #477 added `standalone-installables`, which catches the first. This task catches the rest by doing what a new contributor does: install, then launch.

**Files:**
- Create: `.github/workflows/macos-cold-install.yml`

**Interfaces:**
- Consumes: the `--features populi` install command from Task 1; the `png_icons_are_8_bit` test from PR #477.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the workflow**

Create `.github/workflows/macos-cold-install.yml`:

```yaml
name: macOS Cold Install
on:
  pull_request:
    paths:
      - 'crates/**'
      - 'scripts/install.sh'
      - 'Cargo.toml'
      - 'Cargo.lock'
      - 'rust-toolchain.toml'
  workflow_dispatch:
  schedule:
    - cron: '0 5 * * 1'

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

jobs:
  cold-install:
    name: Cold install + launch (macOS)
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      # The CLI itself.
      - name: Build and install vox
        run: cargo install --path crates/vox-cli --locked --debug

      # Mesh lives behind a non-default feature; a bare install silently omits it.
      - name: Install vox-ml-cli with the populi feature
        run: cargo install --path crates/vox-ml-cli --features populi --locked --debug

      - name: Orchestrator daemon (Axis spawns it)
        run: cargo install --path crates/vox-orchestrator-d --locked --debug

      - name: Delegated subcommands resolve
        run: |
          vox-ml-cli --help | grep -q 'populi'
          vox populi --help

      # Asset guards: 16-bit PNGs abort the app at launch on macOS only.
      - name: Axis prerequisites
        run: cargo test -p vox-gui --test gui_tauri_prereqs

      - name: Build the Axis frontend
        run: |
          corepack enable
          cd crates/vox-gui/ui
          pnpm install --frozen-lockfile
          pnpm build

      - name: Build Axis
        run: cargo build -p vox-gui

      # A non-unwinding panic in did_finish_launching aborts before any window
      # appears, so "the process is still alive" is the assertion that matters.
      - name: Launch Axis headless and assert it survives startup
        run: |
          ./target/debug/vox-gui > /tmp/axis.log 2>&1 &
          PID=$!
          sleep 20
          if ! ps -p $PID > /dev/null; then
            echo "::error::Axis exited during startup"
            cat /tmp/axis.log
            exit 1
          fi
          kill $PID
          if grep -q 'non-unwinding panic' /tmp/axis.log; then
            echo "::error::Axis hit a non-unwinding panic"
            cat /tmp/axis.log
            exit 1
          fi

      - name: Graphify cache builds
        run: |
          vox graph refresh --auto
          vox graph status
```

- [ ] **Step 2: Validate it parses and act can enumerate it**

```bash
uv run --with pyyaml python -c "import yaml; d=yaml.safe_load(open('.github/workflows/macos-cold-install.yml')); print(list(d['jobs'].keys()))"
act --list --workflows .github/workflows/macos-cold-install.yml
```

Expected: prints `['cold-install']`. `act` will list the job but **cannot run it** — `.actrc` deliberately refuses to map `macos-latest` onto a Linux image. That is correct; do not "fix" it by adding a mapping.

- [ ] **Step 3: Dry-run the equivalent steps locally**

Run each `run:` block by hand on this Mac, in order, from a clean `target/`. The launch step is the one that matters — it is the only automated check that would have caught the icon abort.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/macos-cold-install.yml
git commit -m "Add a macOS cold-install and Axis launch CI lane"
```

---

### Task 8: Doctor checks for the local runner and mesh prerequisites

`vox doctor` is where new installs look. It currently says nothing about the local CI runner or about mesh's feature-gated binary, so both fail at use time instead of at diagnosis time.

**Files:**
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs` (beside the `Graphify cache` check added in #477)
- Test: `crates/vox-cli/tests/doctor_runner_checks.rs` (create)

**Interfaces:**
- Consumes: the remediation string from Task 1.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli/tests/doctor_runner_checks.rs`:

```rust
//! Mesh's CLI lives behind a non-default feature and the local runner needs a
//! Docker daemon that is not Docker Desktop. Both currently fail at use time;
//! doctor is where a new install should learn about them.

#[test]
fn doctor_names_the_populi_feature_in_its_remediation() {
    let src = include_str!(
        "../src/commands/diagnostics/doctor/checks_standard/tail.rs"
    );
    assert!(
        src.contains("--features populi"),
        "doctor must tell users that mesh needs the populi feature"
    );
}

#[test]
fn doctor_checks_the_local_act_runner() {
    let src = include_str!(
        "../src/commands/diagnostics/doctor/checks_standard/tail.rs"
    );
    assert!(
        src.contains("Local CI runner (act)"),
        "doctor must report local runner readiness"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-cli --test doctor_runner_checks`
Expected: both FAIL.

- [ ] **Step 3: Add both checks**

In `tail.rs`, immediately after the `Graphify cache` block:

```rust
    // Mesh delegates to `vox-ml-cli`, whose `populi` subcommand is behind a
    // non-default feature — a bare `cargo install` yields a binary without it.
    let ml_cli = which::which("vox-ml-cli").ok();
    let populi_ok = match &ml_cli {
        Some(p) => std::process::Command::new(p)
            .arg("--help")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("populi"))
            .unwrap_or(false),
        None => false,
    };
    checks.push(Check {
        name: "Mesh CLI (vox populi)".to_string(),
        pass: populi_ok,
        detail: if populi_ok {
            "vox-ml-cli exposes `populi`".to_string()
        } else {
            "vox-ml-cli missing or built without mesh — run: \
             cargo install --path crates/vox-ml-cli --features populi"
                .to_string()
        },
    });

    // Local CI mirroring. Docker Desktop is deliberately not used on any host;
    // macOS uses colima, Windows uses WSL2-native docker-ce.
    let act_ok = which::which("act").is_ok();
    checks.push(Check {
        name: "Local CI runner (act)".to_string(),
        pass: true, // optional tooling — informational only
        detail: if act_ok {
            "act present — `vox ci pre-push --act` available".to_string()
        } else {
            "act not installed (optional) — brew install act (macOS; \
             needs colima, not Docker Desktop)"
                .to_string()
        },
    });
```

`which` is already a dependency of `vox-cli-ci`; if it is not in `vox-cli`'s `Cargo.toml`, add it from the workspace (`which = { workspace = true }`) rather than shelling out to `command -v`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-cli --test doctor_runner_checks`
Expected: PASS.

- [ ] **Step 5: Verify against the real machine**

```bash
cargo build -p vox-cli
./target/debug/vox doctor 2>&1 | grep -E 'Mesh CLI|Local CI runner'
```

Expected: `Mesh CLI` passes once Task 1's install has run; `Local CI runner` reports act present.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs \
        crates/vox-cli/tests/doctor_runner_checks.rs
git commit -m "Report mesh feature and local runner readiness in vox doctor"
```

---

### Task 9: Live `act` run and colima sizing guidance

The `act` + colima path was verified on 2026-09-04 only as far as `act -n` (dry run). A live run has never executed — it pulls a ~1 GB image and compiles Rust in-container, and until it runs once the local lane cannot be relied on.

**Files:**
- Modify: `docs/src/ci/alternatives-and-local-mirroring.md` (the macOS paragraph added in #477)

**Interfaces:**
- Consumes: `.actrc`'s `--container-architecture linux/amd64` pin from #477.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Confirm the daemon is sized and Rosetta is live**

```bash
colima list
docker info --format 'CPUs={{.NCPU}} Mem={{.MemTotal}}'
docker run --rm --platform linux/amd64 alpine uname -m
```

Expected: `x86_64` from the last command. If it prints `aarch64`, the architecture pin is not being applied; if it hangs or takes minutes, Rosetta is off — restart with `colima start --vm-type vz --vz-rosetta`.

- [ ] **Step 2: Execute one real job end to end**

```bash
time act --workflows .github/workflows/cross-platform-check.yml \
         --job standalone-installables 2>&1 | tee /tmp/act-live.log
```

Expected: the job runs to `🏁 Job succeeded`. Record the wall-clock time.

- [ ] **Step 3: Record the measured result**

Replace the speculative wording in the macOS paragraph of `docs/src/ci/alternatives-and-local-mirroring.md` with the measured wall-clock time and image size from Step 2, and note any step that failed under emulation. If the run failed, document the failure and its workaround rather than deleting the paragraph — a known-broken lane documented is better than an untested lane implied to work.

- [ ] **Step 4: Commit**

```bash
git add docs/src/ci/alternatives-and-local-mirroring.md
git commit -m "Record measured act + colima timings for the macOS local lane"
```

---

## Self-Review

**Task ordering.** Task 1 is the critical path and should land first: mesh is currently **unbuildable**, and its Step 6 (compiling non-default features in CI) is the gate that would have caught both this defect and the `vox-gamify` one. Tasks 2, 4, 5, 8 are independent and can run in any order. Task 3 is blocked on an owner decision and must not hold up the rest. Task 7 consumes Task 1's fix — its workflow installs `--features populi`, which fails until Task 1 lands. Task 9 depends only on PR #477.

**Spec coverage.** Every open item from the session is mapped: `vox populi` → Task 1; CUDA `.so` → Task 2; ACI `powershell` default → Task 3; `Workspace Registration` → Task 4; the 306-finding report → Tasks 5 and 6; missing macOS E2E → Task 7; runner/mesh invisibility in doctor → Task 8; untested `act` → Task 9. Already landed in PR #477 and therefore *not* re-planned: the `vox-gamify` feature gate, the 16-bit icons, the `voxup` release lookup and ARM target handling, `import-env` empty values, the pnpm 11 override loss, the sccache false negative, the two dead doctor remediations, the Graphify doctor check, the AGENTS.md Graphify section, and the pristine-`.zshrc` PATH bootstrap.

**Placeholder scan.** No `TBD`/`TODO`/"similar to Task N". Task 3 is deliberately gated on an owner decision rather than guessing a contract value — that is a stated blocker with three concrete options and full code for the recommended one, not a placeholder. Tasks 5 and 6 depend on counts measured at execution time; both include the command that produces the number and a rule for what to do with it.

**Type consistency.** `ForbiddenPatternRule` field names in Task 5 match the struct as it exists at `crates/vox-arch-check/src/forbidden_patterns.rs:418-431`. `Check { name, pass, detail }` in Tasks 4 and 8 matches the doctor struct used throughout `tail.rs`. The install string `cargo install --path crates/vox-ml-cli --features populi` is byte-identical in Tasks 1, 7, and 8. `ARTIFACT` in Task 2 keeps its existing name and `&'static str` type.

**Known risk.** Task 5's rules will flag hits that Tasks 1–4 do not fix; Step 5 requires triaging each to either a real fix or an annotated allow. If the Step 1 baseline is large, split per rule — landing a gate that the tree cannot pass is worse than landing nothing.
