# macOS Compatibility Audit & Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the macOS defects surfaced by the 2026-09-04 MacBook bring-up and the seven-track audit that followed, and land the gates that would have caught them — so the next Mac clone is a non-event.

**Architecture:** Six layers, strictly ordered. Layer 0 lands the CI gate first, because every other defect below is downstream of one missing check. Layers 1–2 repair what that gate exposes. Layer 3 makes `vox doctor` truthful. Layers 4–6 gate the pattern, extend existing CI lanes, and collapse the install docs.

**Tech Stack:** Rust 1.96.0 (pinned by `rust-toolchain.toml`), clap, Tauri 2, `vox arch-check` (rules in `docs/src/architecture/layers.toml`), GitHub Actions, `act` + colima, Graphify (`vox graph`).

**Spec:** No separate spec. This implements findings reproduced on macOS 26.5 / aarch64 on 2026-09-04, recorded in `~/dev/memory/vox-macos-setup-blockers.md`, `vox-single-installer-gaps.md`, `vox-graphify-code-intelligence.md`, and `vox-local-ci-docker-macos.md`. PR vox-foundation/vox#477 landed the first wave.

---

## The standing rule

Every defect in this plan — and three of the fixes shipped in #477 — share one shape: **something reported success without anyone verifying the artifact.**

- `vox plugin install` printed `✓ Installed plugin 'mens-candle-metal' (2 files)` having copied no dylib at all.
- A `cargo install … | tail` pipeline returned exit 0 on a failed build, twice.
- A doctor fix compiled cleanly and changed nothing, because its input was always empty.
- A proposed test would have printed `running 0 tests` and exited 0, proving nothing.
- The Graphify check reported `✓ 5 corpus graphs built` while all five were stale.

**So: assert on the artifact, never on the exit code.** After any install, `ls` the thing you installed. After any check fix, re-run the check and read its output. After any test, confirm the run count is non-zero. Every task below encodes this; do not shortcut it.

Corollary for pipelines: `cmd | tail` exits with `tail`'s status. Use `cmd > /tmp/out.log 2>&1; echo $?`.

## Global Constraints

- **Toolchain:** Rust `1.96.0` from `rust-toolchain.toml`. Note `dtolnay/rust-toolchain@stable` sets `RUSTUP_TOOLCHAIN=stable` in `$GITHUB_ENV`, which **overrides the pin** — the repo uses `@stable` for check lanes and `@master` + `toolchain: "1.96.0"` for artifact-producing lanes (`release-installers.yml:22-26`).
- **Published targets:** `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `x86_64-apple-darwin`, `aarch64-apple-darwin`. Never emit a URL for any other triple.
- **No new runtime dependencies.**
- **Installer parity:** `scripts/install.sh` ≡ `docs-astro/public/voxup`, `scripts/install.ps1` ≡ `docs-astro/public/voxup.ps1`, byte-for-byte (`documented_install_urls_are_served`). Edit one, `cp` to the other in the same commit.
- **Docs frontmatter** is required for new `.md` under `docs/src/` only. Editing an existing file needs none. `docs/superpowers/` is exempt.
- **Repo conventions:** `actions/checkout@v7`; every workflow sets `timeout-minutes`; `shell: bash` on multi-line `run:` blocks; `set -euo pipefail` inside them.
- **Concurrency warning:** do not run two `cargo check` audits against this worktree at once — a parallel edit produced a phantom failure during the audit. Use `git worktree` for concurrent work.

## Execution Protocol

**Branch & PR.** All tasks land on `fix/macos-bringup-and-installer` (PR #477). If #477 has merged, branch `fix/macos-compat-wave-2` from `main`. Never push to `main`.

**Per-task exit gate.** After the task's commit, run the affected crate's *full* suite — the per-test filters inside each task prove new behavior, not absence of regressions:
- Tasks 1, 6, 7, 8 → `cargo test -p vox-cli`
- Task 2 → `cargo check -p vox-plugin-mens-candle-metal -p vox-plugin-mens-candle-cuda`
- Task 3 → `cargo test -p vox-ml-cli --features populi`
- Task 5 → `cargo test -p vox-orchestrator-mcp`
- Task 9 → `cargo test -p vox-arch-check && cargo run -p vox-arch-check --quiet`

**Rollback.** One commit per task; `git revert <sha>`. Never amend or force-push — #477's review history depends on the branch tip.

**Ordering.** Task 0 first. Tasks 1–2 are what Task 0 exposes. Tasks 6 and 7 both edit `tail.rs`: land 6 before 7. Everything else is independent.

---

### Task 0: Gate non-default features in CI

Three separate defects in this plan are the same bug: a crate under-declares a feature it uses, and `cargo check --workspace` hides it because **workspace builds unify features across the whole graph**. PR #477 fixed one instance (`vox-gamify`) and treated the class as closed. It was not.

Verified 2026-09-04 — all three fail standalone and pass under `--workspace`:

| Crate | Error | Reached by |
|---|---|---|
| `vox-ml-cli --features populi` | `E0063` missing 6 fields | `vox populi` (mesh) |
| `vox-plugin-mens-candle-metal` | `E0599 connect_default` | `vox plugin install mens-candle-metal` |
| `vox-plugin-mens-candle-cuda` | `E0599 connect_default` | `plugin_heal.rs:201` auto-heal |

Three existing lanes each miss it: `cross-platform-check.yml:80` is `--workspace` (defaults only); `ci.yml:1385` excludes `vox-ml-cli` and both metal crates whenever `nvcc` is absent, which is always; `ci.yml:1790` runs `--all-features` but is not in `ci-summary`'s `needs`, so it gates nothing.

**Land the gate before the fixes** so each subsequent task has a failing check to turn green.

**Files:**
- Modify: `.github/workflows/cross-platform-check.yml` (the `standalone-installables` job, line ~144)

**Interfaces:**
- Produces: a CI job that fails until Tasks 1 and 2 land. That is intended.

- [ ] **Step 1: Reproduce all three failures**

```bash
cd /Users/brbrainerd/dev/vox
for c in vox-plugin-mens-candle-metal vox-plugin-mens-candle-cuda; do
  cargo check -p "$c" > "/tmp/$c.log" 2>&1; echo "$c => $?"
done
cargo check -p vox-ml-cli --features populi > /tmp/populi.log 2>&1; echo "populi => $?"
```

Expected: all three non-zero. Paste the three error lines into the commit body.

- [ ] **Step 2: Widen the standalone job**

Replace the `cargo check (standalone, per-crate)` step's `run:` block:

```yaml
      # `-p` with no `--workspace`: each is a separate feature resolution, which is
      # what `cargo install --path` does. `--all-targets` because plain `cargo check`
      # skips tests and benches, where feature-gated code also rots.
      - name: cargo check (standalone, per-crate)
        shell: bash
        run: |
          set -euo pipefail
          cargo check --all-targets -p vox-ml-cli
          cargo check --all-targets -p vox-ml-cli --features populi
          cargo check --all-targets -p vox-gamify
          cargo check --all-targets -p vox-orchestrator-d
          # Installed by `vox plugin install`; both under-declare vox-db/host-integration.
          cargo check --all-targets -p vox-plugin-mens-candle-metal
          cargo check --all-targets -p vox-plugin-mens-candle-cuda
```

- [ ] **Step 3: Verify the workflow parses and act enumerates it**

```bash
uv run --no-project --with pyyaml python -c "import yaml;d=yaml.safe_load(open('.github/workflows/cross-platform-check.yml'));print(list(d['jobs'].keys()))"
act --list --workflows .github/workflows/cross-platform-check.yml
```

Expected: `['cross-check', 'standalone-installables']`; act lists both. Only the `ubuntu-latest` matrix leg is runnable locally — `.actrc` deliberately refuses to map `macos-latest`.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/cross-platform-check.yml
git commit -m "Compile non-default features and all targets standalone in CI"
```

---

### Task 1: Repair the rotted `populi` feature

`cargo build -p vox-ml-cli --features populi` fails: `WorkerDonationPolicy` gained six fields and its only feature-gated consumer never picked them up. Mesh has been **unbuildable**, not merely unshipped.

Two earlier readings of this were wrong and must not be repeated: it is not "dead CLI surface" (the variant exists at `crates/vox-ml-cli/src/main.rs:27-31`), and it is not fixed by simply passing `--features populi` (that is the invocation that fails).

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/populi_lifecycle.rs:110`
- Modify: `crates/vox-cli/src/main.rs:115`
- Modify: `CONTRIBUTING.md`, `docs/src/how-to/populi-quickstart.md:14`, `docs/src/reference/cli.md:154,158,160`
- Test: `crates/vox-cli/tests/ml_delegation_hint.rs` (create)

**Interfaces:**
- Produces: the string `cargo install --path crates/vox-ml-cli --features populi`. CI and local-verify steps append `--locked --debug`; the prefix is identical everywhere.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli/tests/ml_delegation_hint.rs`. (`include_str!` resolves relative to the containing file, so `../src/main.rs` is `crates/vox-cli/src/main.rs` — verified.)

```rust
//! The hint printed when `vox-ml-cli` is missing must name a command that
//! actually produces the delegated subcommand. `populi` is a non-default
//! feature, so a bare install yields a binary without it and the user retries
//! the same failing command.

#[test]
fn ml_cli_install_hint_enables_the_populi_feature() {
    let src = include_str!("../src/main.rs");
    let hint = src
        .lines()
        .find(|l| l.contains("cargo install --path crates/vox-ml-cli"))
        .expect("the delegation failure message must name the install command");
    assert!(
        hint.contains("--features populi"),
        "install hint must enable the `populi` feature, got: {hint}"
    );
}
```

- [ ] **Step 2: Run it and confirm it fails**

```bash
cargo test -p vox-cli --test ml_delegation_hint
```

Expected: FAIL, `install hint must enable the "populi" feature`. Confirm the output says `running 1 test` — a `0 tests` line means the filter matched nothing and proves nothing.

- [ ] **Step 3: Fix the initializer**

`WorkerDonationPolicy` has **no `Default` impl**, so every field must be named. In `crates/vox-ml-cli/src/commands/populi_lifecycle.rs`, add after `redundancy: None,` (line 110 — *not* after `slots`, which is a 20-line `.collect()` chain ending at line 89):

```rust
                // Defaults match the two other policy parsers (vox-mesh-policy
                // and vox-scaling-policy, both `false`) and the `#[serde(default)]`
                // on the fields themselves. `accepts_inference_workloads` is not
                // read by any admission path *yet* — a2a.rs gates on
                // public_mesh_opt_in/min_priority/slots — so defaulting it true
                // here would silently make every `vox populi up` node an inference
                // donor the day a planner starts honoring it.
                accepts_inference_workloads: false,
                accepts_training_workloads: false,
                accepts_sensitive_training_data: false,
                cuda_tier: 0,
                metal_tier: 0,
                vram_min_gb: 0,
```

Read `crates/vox-mesh-types/src/donation_policy.rs` first; if the field set has drifted again, match the struct. **Do not** derive `Default` and use `..Default::default()` — exhaustive initialization is the only reason this drift was ever reported.

- [ ] **Step 4: Fix the delegation hint**

At `crates/vox-cli/src/main.rs:115`, replace:

```rust
                    eprintln!("Please run: cargo install --path crates/vox-ml-cli");
```

with:

```rust
                    eprintln!(
                        "Please run: cargo install --path crates/vox-ml-cli --features populi"
                    );
```

- [ ] **Step 5: Verify the feature compiles and mesh answers**

```bash
cargo build -p vox-ml-cli --features populi > /tmp/populi.log 2>&1; echo "EXIT=$?"
cargo install --path crates/vox-ml-cli --features populi --locked --debug > /tmp/inst.log 2>&1; echo "EXIT=$?"
if vox-ml-cli --help | grep -q populi; then echo "populi present"; else echo "MISSING"; exit 1; fi
vox populi status
cargo test -p vox-cli --test ml_delegation_hint
```

Expected: both `EXIT=0`; `populi present`; `vox populi status` returns real output; test passes. **Stop condition:** if the build still fails after the six fields land, a second consumer has rotted — record the new error and file it separately rather than widening this task.

- [ ] **Step 6: Correct the docs that name the wrong command**

- `CONTRIBUTING.md` — ML/mesh row → `cargo install --path crates/vox-ml-cli --features populi`, and note `populi` is not a default feature.
- `docs/src/how-to/populi-quickstart.md:14` — says `cargo build -p vox-ml-cli --features populi`. A *build* does not put `vox-ml-cli` on `PATH`, and `crates/vox-cli/src/main.rs:95` delegates via a bare `Command::new("vox-ml-cli")` PATH lookup, so the quickstart cannot work as written. Change to `cargo install`.
- `docs/src/reference/cli.md:154,158,160` — three references to `cargo build -p vox-cli --features populi`. **vox-cli has no `populi` feature** (it lives on vox-ml-cli). Line 160 also names `mesh-nvml-probe`, which exists on no crate. Correct all three.

Verify: `grep -rn -- "--features populi" CONTRIBUTING.md docs/src/how-to/populi-quickstart.md docs/src/reference/cli.md` shows only `vox-ml-cli`, never `vox-cli`.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-ml-cli/src/commands/populi_lifecycle.rs crates/vox-cli/src/main.rs \
        crates/vox-cli/tests/ml_delegation_hint.rs CONTRIBUTING.md \
        docs/src/how-to/populi-quickstart.md docs/src/reference/cli.md
git commit -m "Repair the populi feature and correct its install docs"
```

---

### Task 2: Fix the GPU plugin crates' feature declaration

`vox-plugin-mens-candle-metal` and `vox-plugin-mens-candle-cuda` both call `vox_db::VoxDb::connect_default()` at `src/candle_qlora_train/db_thread.rs:32`, which is `#[cfg(all(feature = "local", feature = "host-integration"))]` (`crates/vox-db/src/facade/connect.rs:20-22`). Both declare `vox-db = { workspace = true }` — defaults only.

This is character-for-character the `vox-gamify` bug from #477. It matters on macOS specifically: `CONTRIBUTING.md:34` tells Apple Silicon users to run `vox plugin install mens-candle-metal --yes`, and `plugin_heal.rs:201` shells out to `cargo build -p vox-plugin-mens-candle-cuda --release --features cuda` — a standalone `-p` resolve that hits this before it ever reaches `nvcc`.

**Files:**
- Modify: `crates/vox-plugin-mens-candle-metal/Cargo.toml:17`
- Modify: `crates/vox-plugin-mens-candle-cuda/Cargo.toml:17`

- [ ] **Step 1: Confirm both fail and the sibling precedent**

```bash
cargo check -p vox-plugin-mens-candle-metal > /tmp/m.log 2>&1; echo "metal => $?"
cargo check -p vox-plugin-mens-candle-cuda  > /tmp/c.log 2>&1; echo "cuda  => $?"
grep -rn 'vox-db.*host-integration' crates/*/Cargo.toml
```

Expected: both non-zero with `E0599 connect_default`; the grep shows `vox-cli`, `vox-sql`, and `vox-gamify` as precedent.

- [ ] **Step 2: Apply the same fix to both**

In each `Cargo.toml`, line 17:

```toml
vox-db = { workspace = true, features = ["host-integration"] }
```

- [ ] **Step 3: Verify both build standalone**

```bash
cargo check -p vox-plugin-mens-candle-metal > /tmp/m.log 2>&1; echo "metal => $?"
cargo check -p vox-plugin-mens-candle-cuda  > /tmp/c.log 2>&1; echo "cuda  => $?"
```

Expected: both `0`. Task 0's CI job now passes for these two crates.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-plugin-mens-candle-metal/Cargo.toml crates/vox-plugin-mens-candle-cuda/Cargo.toml
git commit -m "Enable vox-db host-integration in both GPU plugin crates"
```

---

### Task 3: Make `vox plugin install` install something

`vox plugin install` reports success having installed nothing loadable. Verified on this machine after a "successful" install:

```
~/Library/Application Support/vox/plugins/mens-candle-metal/0.1.0/
  Cargo.toml   Plugin.toml          # and nothing else
$ find ~/Library/Application\ Support/vox/plugins -name '*.dylib' | wc -l
0
```

`crates/vox-cli/src/commands/plugin/install.rs:70-89` copies top-level *files* from the source dir and prints `✓ Installed plugin … (N files)`; it never builds or copies the cdylib. The workspace-local fallback at `install.rs:158-171` routes a dev-checkout install straight into that path. The presence check that should have caught it is itself broken: `plugin/info.rs:45-58` builds the filename as `format!("{}.{}", id, ext)` → `mens-candle-metal.dylib`, while the manifest and `vox_plugin_types::plugin_artifact_filename` say `libvox_plugin_mens_candle_metal.dylib`. Dead code on every platform.

Depends on Task 2 — the Metal plugin cannot be built until its manifest is fixed.

**Files:**
- Modify: `crates/vox-cli/src/commands/plugin/install.rs:70-89`
- Modify: `crates/vox-cli/src/commands/plugin/info.rs:45-58`
- Modify: `crates/vox-plugin-populi-mesh/Plugin.toml:20-22`
- Test: `crates/vox-cli/src/commands/plugin/install.rs`, inline `#[cfg(test)]`

- [ ] **Step 1: Capture the current broken state**

```bash
vox plugin install mens-candle-metal --yes > /tmp/pi.log 2>&1; echo "EXIT=$?"
ls -la ~/Library/Application\ Support/vox/plugins/mens-candle-metal/0.1.0/
find ~/Library/Application\ Support/vox/plugins -name '*.dylib' | wc -l
```

Expected: exit 0, a success message, and **0 dylibs**. Paste this into the commit body — it is the whole justification.

- [ ] **Step 2: Add the macOS artifact keys to the mesh plugin**

`crates/vox-plugin-populi-mesh/Plugin.toml:20-22` declares only `windows-x86_64` and `linux-x86_64`. `vox-plugin-host/src/lib.rs:170-176` does `artifacts.get(triple)` and errors with *"no artifact for target triple 'macos-aarch64'"* — so mesh cannot load on any Mac even after Task 1. The transport is pure Rust/axum; there is no platform reason for the omission (unlike `mens-candle-cuda` and `nvml-probe`, which are legitimately non-macOS).

```toml
[plugin.payload.code.artifacts]
"windows-x86_64" = "vox_plugin_populi_mesh.dll"
"linux-x86_64"   = "libvox_plugin_populi_mesh.so"
"macos-aarch64"  = "libvox_plugin_populi_mesh.dylib"
"macos-x86_64"   = "libvox_plugin_populi_mesh.dylib"
```

- [ ] **Step 3: Write the failing test**

Add to `crates/vox-cli/src/commands/plugin/install.rs`:

```rust
#[cfg(test)]
mod artifact_install_tests {
    use super::*;

    /// A code plugin whose manifest declares an artifact for this triple must not
    /// report success unless that artifact landed. The installer used to copy only
    /// top-level files and print "✓ Installed", leaving a plugin that dlopen fails.
    #[test]
    fn install_rejects_a_code_plugin_with_no_artifact() {
        let src = tempfile::tempdir().expect("src");
        std::fs::write(
            src.path().join("Plugin.toml"),
            r#"
[plugin]
id = "demo"
version = "0.1.0"
[plugin.payload.code.artifacts]
"macos-aarch64" = "libdemo.dylib"
"linux-x86_64"  = "libdemo.so"
"#,
        )
        .expect("write manifest");
        let dest = tempfile::tempdir().expect("dest");

        let err = install_from_path(src.path(), dest.path())
            .expect_err("must fail when the declared artifact is absent");
        let msg = err.to_string();
        assert!(
            msg.contains("artifact") || msg.contains("dylib") || msg.contains("libdemo"),
            "error must name the missing artifact, got: {msg}"
        );
    }
}
```

Match `install_from_path`'s real signature and error type before running — if it takes different arguments, adapt the call, not the assertion.

- [ ] **Step 4: Run it and confirm it fails**

```bash
cargo test -p vox-cli --lib artifact_install_tests
```

Expected: FAIL — the install currently succeeds. Confirm `running 1 test`.

- [ ] **Step 5: Require the artifact**

In `install_from_path`, after copying files: if the manifest has a `[plugin.payload.code.artifacts]` table, resolve the entry for the current triple via `vox_plugin_types::plugin_artifact_filename` (the declared SSOT — do not reconstruct the name), and fail if that file is not present in the destination. Build it if the source is a workspace crate; otherwise return an actionable error naming the artifact and the triple.

- [ ] **Step 6: Fix the presence check**

In `plugin/info.rs:45-58`, replace the `format!("{}.{}", id, ext)` reconstruction with a manifest lookup through the same helper, so `Native lib` reports reality on every platform.

- [ ] **Step 7: Verify end to end**

```bash
cargo test -p vox-cli --lib artifact_install_tests
cargo build -p vox-cli
./target/debug/vox plugin install mens-candle-metal --yes > /tmp/pi2.log 2>&1; echo "EXIT=$?"
find ~/Library/Application\ Support/vox/plugins -name '*.dylib'
```

Expected: test passes; either a dylib now exists, or the install **fails loudly**. Both are correct; silent success is not.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-cli/src/commands/plugin/install.rs crates/vox-cli/src/commands/plugin/info.rs \
        crates/vox-plugin-populi-mesh/Plugin.toml
git commit -m "Require the declared artifact on plugin install; add macOS mesh keys"
```

---

### Task 4: Require a macOS artifact key in the plugin CI gate

Task 3 fixes one manifest. `crates/vox-cli-ci/src/plugin_surface.rs:233-253` validates that every key *present* is a known triple and that filenames match the cdylib rule — but never that a macOS key *exists*. That is how the mesh plugin shipped without one.

**Files:**
- Modify: `crates/vox-cli-ci/src/plugin_surface.rs:233-253`
- Test: same file, inline `#[cfg(test)]`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn code_plugins_must_declare_a_macos_artifact() {
    let manifest = r#"
[plugin]
id = "demo"
[plugin.payload.code.artifacts]
"windows-x86_64" = "demo.dll"
"linux-x86_64"   = "libdemo.so"
"#;
    let errs = validate_plugin_manifest_str("demo", manifest);
    assert!(
        errs.iter().any(|e| e.contains("macos")),
        "a code plugin with no macos-* artifact must be rejected, got: {errs:?}"
    );
}
```

Use the real validator entry point from this module; adapt the call, not the assertion.

- [ ] **Step 2: Run it and confirm it fails**

`cargo test -p vox-cli-ci code_plugins_must_declare_a_macos_artifact` → FAIL, `running 1 test`.

- [ ] **Step 3: Add the requirement**

Require at least one `macos-*` key for every code/composite plugin, with an explicit opt-out list for the legitimately non-macOS plugins — `mens-candle-cuda`, `nvml-probe` — carrying a one-line reason each.

- [ ] **Step 4: Verify and commit**

```bash
cargo test -p vox-cli-ci
git add crates/vox-cli-ci/src/plugin_surface.rs
git commit -m "Require a macOS artifact key for code plugins"
```

---

### Task 5: Decide the ACI shell backend for POSIX hosts

`crates/vox-orchestrator-mcp/src/aci/envelope.rs:47` returns `"powershell"` on every platform. **Blocked on an owner decision** — `contracts/aci/agent-computer-interface.v1.yaml` defines only `contract_first`, `powershell`, `nushell`. There is no POSIX adapter, so a correct macOS default cannot be invented in code.

- **(A)** Default to `contract_first` when unspecified — the SSOT already calls it "shell-agnostic". Smallest change, no contract edit. *Recommended.*
- **(B)** Add a `posix` adapter to the YAML SSOT, both JSON schemas, and the normalizer. Most correct, largest blast radius.
- **(C)** Keep `powershell`, document it as a telemetry label. Zero code change; needs a comment so this is not re-filed.

- [ ] **Step 1: Get the decision**

Ask the ACI owner (from `CODEOWNERS` for `contracts/aci/`; if absent, ask the user who to tag). Record the answer in the commit body. **If no answer this session, stop and move on** — this is not on the critical path.

- [ ] **Step 2 (option A): Write the failing test**

```rust
#[test]
fn aci_shell_backend_defaults_to_contract_first_without_args() {
    let v = super::aci_shell_backend_for_tool("vox_run_shell", None);
    assert_eq!(v, serde_json::Value::String("contract_first".into()));
}
```

- [ ] **Step 3 (A): Confirm it fails**

`cargo test -p vox-orchestrator-mcp aci_shell_backend_defaults_to_contract_first_without_args` → FAIL (left `"powershell"`).

- [ ] **Step 4 (A): Change both default sites**

`envelope.rs:47` (no args) **and** `envelope.rs:55` (`unwrap_or("powershell")`, args present without a `backend` key). Changing only the first leaves `{"cmd":"ls"}` still mislabeled — a half-applied fix. Add `"contract_first" => "contract_first"` to the normalizer match; leave the explicit `nushell`/`powershell` arms untouched.

- [ ] **Step 5 (A): Update the test that encodes the old default**

`aci_shell_backend_for_run_shell_default_pwsh` at `envelope.rs:168-176` asserts `"powershell"` for exactly this case. It **will** fail — this is a certainty, not a risk. Rename it to `aci_shell_backend_for_run_shell_defaults_to_contract_first`, change its assertion, and say so in the commit body. `aci_shell_backend_for_run_shell_nushell_when_requested` must still pass unchanged.

- [ ] **Step 6: Verify and commit**

```bash
cargo test -p vox-orchestrator-mcp
git add crates/vox-orchestrator-mcp/src/aci/envelope.rs
git commit -m "Default the ACI shell backend to contract_first when unspecified"
```

---

### Task 6: Make `vox doctor` truthful

Six defects, all verified. Doctor is where new installs look, and a check that is wrong in either direction trains people to ignore all of them.

| Check | Defect |
|---|---|
| `Graphify cache` | Reports `✓ 5 built` while `vox graph status` says all 5 stale. Counts `graph.json` files; hardcodes `.vox/cache/graphify` though a move to `.vox/cache/vox-graph` is partly landed; miscounts virtual corpora. |
| `Workspace Registration` | Reads `project.vox-workspace.path`, which **nothing writes**. Remediation now says `vox repo init`, which writes `.vox/repositories.yaml` — a *different* artifact, so following the advice still leaves it red. |
| `tier dep: model-weights` | Says `run: vox mens pull`. **`pull` is not a `vox mens` subcommand.** |
| `Vox Config` | Says `run: vox login`. `login` writes `~/.vox/login.toml`; the check reads `~/.vox/config.toml`. Only doctor's own auto-heal writes it. |
| `vox-lsp binary` | Says `cargo build -p vox-lsp --release`, but the check looks next to `current_exe()` (`target/debug/`). Following the advice leaves it red. |
| `--probe` | `doctor/mod.rs:104-110` fails on *any* red row, and `--probe` is the `HEALTHCHECK` in `Dockerfile:43`. In the runtime image six checks always fail, so those containers can never be healthy. |

**Files:**
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs`
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tier_deps.rs:58`
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/mod.rs:104-110`

- [ ] **Step 1: Write the remediation lint — the test that catches this class**

This one test replaces every string-grep test and keeps catching new dead advice. Add to `checks_standard/mod.rs`:

```rust
#[cfg(test)]
mod remediation_tests {
    /// Every `vox …` command doctor recommends must exist in the clap tree.
    /// Five dead remediations shipped simultaneously (`vox setup`,
    /// `vox login --registry`, `vox mens pull`, and two that name a real command
    /// which does not affect the checked artifact) because nothing asserted this.
    #[tokio::test]
    async fn every_remediation_names_a_real_vox_command() {
        let mut checks = Vec::new();
        super::tail::run(false, &mut checks).await;
        let known: std::collections::HashSet<String> = crate::command_catalog::build_catalog()
            .entries
            .iter()
            .map(|e| e.command.clone())
            .collect();
        for c in &checks {
            for cmd in extract_vox_invocations(&c.detail) {
                assert!(
                    known.contains(&cmd),
                    "{}: recommends `{cmd}`, which is not in the clap tree",
                    c.name
                );
            }
        }
    }
}
```

Write `extract_vox_invocations` to scan for `vox ` and take the longest prefix matching a catalog entry. Match `tail::run`'s real signature.

- [ ] **Step 2: Run it and confirm it fails**

`cargo test -p vox-cli --lib remediation_tests` → FAIL, naming at least `vox mens pull`. Confirm `running 1 test`.

- [ ] **Step 3: Fix the four dead remediations**

- `tier_deps.rs:58` → a real `vox mens` subcommand (check `vox-ml-cli mens --help`), or drop the command from the text.
- `Vox Config` → `vox doctor --auto-heal`, the only writer of `config.toml`.
- `vox-lsp binary` → `cargo build -p vox-lsp` (matching the profile the check looks in), or widen the lookup to both profiles.
- `Workspace Registration` → point the *check* at the artifact its cure creates:

```rust
    // `vox repo init` writes `.vox/repositories.yaml`; nothing anywhere writes the
    // old `project.vox-workspace.path` DB key, so the check could never pass and
    // the advice could never cure it. Check what the remediation actually produces.
    let repo_yaml = repo_root.join(".vox/repositories.yaml");
    let reg_pass = tokio::fs::try_exists(&repo_yaml).await.unwrap_or(false);
```

- [ ] **Step 4: Fix the Graphify check with pure classifiers**

Replace the file-count with the freshness API (`vox graph status` runs the full assessment in 0.32s, so cost is not a concern), and resolve the root via `crate::commands::ci::repo_root()` rather than a relative path:

```rust
/// Pure, unit-testable. Missing is a real failure; drift is reported but never
/// fails — `worktree_drift` fires for anyone with uncommitted edits and would be
/// permanent noise.
pub(crate) fn graphify_cache_check(statuses: &[CorpusStatus]) -> Check {
    let missing: Vec<&str> = statuses.iter()
        .filter(|s| !s.graph_exists && !s.warnings.iter().any(|w| w == "virtual_corpus"))
        .map(|s| s.corpus_id.as_str()).collect();
    let stale: Vec<&str> = statuses.iter()
        .filter(|s| s.graph_exists && !s.is_fresh)
        .map(|s| s.corpus_id.as_str()).collect();
    if !missing.is_empty() {
        Check::fail("Graphify cache", format!(
            "{} corpus graph(s) not built ({}) — run: vox graph refresh --auto \
             (code-intelligence queries return nothing until this is built)",
            missing.len(), missing.join(", ")))
    } else if !stale.is_empty() {
        Check::pass("Graphify cache", format!(
            "{} of {} corpus graph(s) stale ({}) — run: vox graph refresh --auto",
            stale.len(), statuses.len(), stale.join(", ")))
    } else {
        Check::pass("Graphify cache", format!("{} corpus graph(s) fresh", statuses.len()))
    }
}

#[cfg(test)]
mod graphify_cache_tests {
    use super::*;

    #[test]
    fn missing_graphs_fail_and_stale_graphs_do_not() {
        // Build CorpusStatus fixtures per the struct in vox_config::graphify.
        assert!(!graphify_cache_check(&[status(false, false)]).pass);
        assert!(graphify_cache_check(&[status(true, false)]).pass);
        assert!(graphify_cache_check(&[status(true, true)]).pass);
    }
}
```

- [ ] **Step 5: Fix the `--probe` health gate**

In `doctor/mod.rs:104-110`, gate `--probe` on a **required-checks subset** rather than `failed > 0`. Required: toolchain identity, docker reachability, VoxDB writability, schema version. Everything repo- or key-dependent is advisory. The contract test in `crates/vox-integration-tests/tests/docker_healthcheck_contract_test.rs` must still pass.

- [ ] **Step 6: Verify against the machine**

```bash
cargo test -p vox-cli --lib remediation_tests graphify_cache_tests
cargo build -p vox-cli
./target/debug/vox doctor 2>&1 | grep -E "Graphify cache|Workspace Registration|model-weights|Vox Config|vox-lsp"
```

Expected: the remediation lint passes; `Graphify cache` now reports **stale**, not a green count. If the `grep` finds nothing, check the real prefix with `./target/debug/vox doctor | head -30` — the rows use two leading spaces.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/src/commands/diagnostics/doctor/
git commit -m "Make doctor checks and their remediations agree"
```

---

### Task 7: Report mesh and local-runner readiness in doctor

Depends on Task 6 (both edit `tail.rs`; land 6 first).

**Files:**
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs`

Note: `which` is **already** a dependency (`crates/vox-cli/Cargo.toml:282`) — no manifest change. `Check` is already in scope in `tail.rs`, and both `which::which` and `std::process::Command` paths are fully qualified, so **no new `use` lines are needed**.

- [ ] **Step 1: Write the failing tests against pure classifiers**

```rust
pub(crate) const MESH_INSTALL_HINT: &str =
    "cargo install --path crates/vox-ml-cli --features populi";

pub(crate) fn mesh_cli_check(populi_available: bool) -> Check { /* Step 3 */ }
pub(crate) fn act_runner_check(act_present: bool) -> Check { /* Step 3 */ }

#[cfg(test)]
mod runner_checks_tests {
    use super::*;

    #[test]
    fn mesh_check_fails_closed_and_names_the_feature() {
        let c = mesh_cli_check(false);
        assert!(!c.pass);
        assert!(c.detail.contains("--features populi"), "{}", c.detail);
    }

    #[test]
    fn mesh_check_passes_when_populi_is_present() {
        assert!(mesh_cli_check(true).pass);
    }

    #[test]
    fn act_is_never_a_hard_failure() {
        assert!(act_runner_check(false).pass);
        assert!(act_runner_check(true).pass);
    }
}
```

- [ ] **Step 2: Confirm they fail**

`cargo test -p vox-cli --lib runner_checks_tests` → FAIL (functions undefined), `running 3 tests`.

- [ ] **Step 3: Implement, with a bounded probe**

Insert **after line 310** — the closing brace of `if in_vox_repo`, *not* inside it. Both checks are machine-scoped, not repo-scoped.

```rust
    // Bounded like the hook-guard probe in build_health.rs: a stale or hung
    // `vox-ml-cli` on PATH must never wedge doctor — `vox doctor --probe` is the
    // root Dockerfile HEALTHCHECK with a 5s budget. Use tokio's Command (already
    // imported); std::process::Command would park a worker thread.
    let populi_ok = match which::which("vox-ml-cli") {
        Ok(bin) => {
            // Ask for the subcommand itself: clap exits non-zero on an unknown
            // subcommand, so this is exact rather than a substring guess against
            // help text that might mention "populi" for unrelated reasons.
            let probe = Command::new(&bin)
                .args(["populi", "--help"])
                .kill_on_drop(true)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            matches!(
                tokio::time::timeout(vox_config::timeouts::D_5S, probe).await,
                Ok(Ok(s)) if s.success()
            )
        }
        Err(_) => false,
    };
    checks.push(mesh_cli_check(populi_ok));
    checks.push(act_runner_check(which::which("act").is_ok()));
```

`act_runner_check` returns `Check::pass` in both branches — optional tooling, per the house `"(optional)"` naming convention.

- [ ] **Step 4: Verify**

```bash
cargo test -p vox-cli --lib runner_checks_tests
cargo build -p vox-cli
./target/debug/vox doctor 2>&1 | grep -E "Mesh CLI|Local CI runner"
```

Expected: `Mesh CLI` passes once Task 1's install has run; `Local CI runner` reports act present.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tail.rs
git commit -m "Report mesh feature and local runner readiness in vox doctor"
```

---

### Task 8: Serialize the env-mutating capability tests

`crates/vox-repository/src/capabilities.rs:270-300` has four tests mutating process-global `VOX_MESH_ADVERTISE_GPU` / `VOX_MESH_DEVICE_CLASS`, with a comment claiming they are "mutated single-threaded" — false under `cargo test`. Reproduced: `--test-threads=1` passes; 5 parallel runs failed twice.

Not a macOS bug, but an 18-core Mac schedules the collision more often, so the new macOS CI legs will flake and be misdiagnosed as a platform problem. Fix before Task 9.

- [ ] **Step 1: Reproduce**

```bash
cargo test -p vox-repository capabilities -- --test-threads=1   # expect pass
for i in 1 2 3 4 5; do cargo test -p vox-repository capabilities > /tmp/r$i.log 2>&1; echo "run $i => $?"; done
```

Expected: serial passes; at least one parallel run fails on `assertion failed: !h.gpu_cuda`.

- [ ] **Step 2: Remove the shared mutable state**

Preferred: refactor `apply_mesh_capability_env` to take the values as parameters so the tests need no process env at all. Fallback: a shared `static MUTEX: Mutex<()>` acquired by all four. Either way, delete the inaccurate "mutated single-threaded" comment.

- [ ] **Step 3: Verify determinism**

```bash
for i in 1 2 3 4 5; do cargo test -p vox-repository capabilities > /tmp/r$i.log 2>&1; echo "run $i => $?"; done
```

Expected: five zeros.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-repository/src/capabilities.rs
git commit -m "Remove the process-env race from the capability tests"
```

---

### Task 9: Gate literal `~/` paths in arch-check

**Scope corrected from the first draft.** Two rules were proposed; one is deleted and the other rewritten:

- `no-hardcoded-tmp-path` is **fully redundant** — `docs/src/architecture/layers.toml:507` already ships `no-hardcoded-abs-path` with `'"(/(tmp|usr|etc|var|home|opt|bin|root)\b|[A-Za-z]:\\)'`, at `error`, tree clean. Do not add it.
- The broad `"~/` pattern is **100% false positives**: 4 surviving hits, 0 real. One is `vox-orchestrator/src/memory/project_file.rs:104`'s `strip_prefix("~/")` — the tilde-*expansion implementation*, i.e. the rule would flag its own fix. And `layers.toml:99` sets `forbidden_pattern = "error"` as a **single global guard over all rules**, so a rule with open violations fails CI for everyone immediately.

Rules live in **TOML**, not Rust — `forbidden_patterns.rs`'s `*_rule()` functions are test fixtures only.

**Files:**
- Modify: `docs/src/architecture/layers.toml` (after the `no-hardcoded-dynlib-ext` block at line 533)
- Test: `crates/vox-arch-check/src/forbidden_patterns.rs`, inside `mod tests`

- [ ] **Step 1: Write the failing tests**

The helper is `scan`, not `scan_rule` (`forbidden_patterns.rs:97`). `write_fixture` and `crate::built_in_walk_prune_names()` are correct as named.

```rust
fn home_tilde_rule() -> ForbiddenPatternRule {
    ForbiddenPatternRule {
        name: "no-literal-home-tilde".into(),
        exempt_tests: true,
        // Anchored to a path constructor. The bare `"~/` form is 100% false
        // positives on this tree — error messages, a CARGO_HOME doc default, and
        // the tilde-expansion helper itself.
        pattern: r#"(PathBuf::from|Path::new)\(\s*"~/"#.into(),
        file_glob: "crates/**/*.rs".into(),
        exempt_files: vec![],
        allow_annotation: Some("// vox-arch-check: allow home-tilde".into()),
        reason: "Rust does not expand `~`; use vox_config::paths::user_home_dir().".into(),
    }
}

#[test]
fn home_tilde_rule_flags_constructor_tilde_paths() {
    let dir = tempfile::tempdir().unwrap();
    write_fixture(&dir, "crates/x/src/a.rs", r#"let p = PathBuf::from("~/.vox/c.jsonl");"#);
    let hits = scan(dir.path(), &home_tilde_rule(), &crate::built_in_walk_prune_names()).unwrap();
    assert_eq!(hits.len(), 1);
}

#[test]
fn home_tilde_rule_ignores_prefix_matching() {
    let dir = tempfile::tempdir().unwrap();
    // The tilde-expansion implementation must not be flagged as the bug it fixes.
    write_fixture(&dir, "crates/x/src/a.rs", r#"if let Some(r) = s.strip_prefix("~/") {}"#);
    let hits = scan(dir.path(), &home_tilde_rule(), &crate::built_in_walk_prune_names()).unwrap();
    assert!(hits.is_empty());
}

#[test]
fn home_tilde_rule_respects_allow_annotation() {
    let dir = tempfile::tempdir().unwrap();
    write_fixture(&dir, "crates/x/src/a.rs",
        "// vox-arch-check: allow home-tilde\nlet p = PathBuf::from(\"~/.vox\");");
    let hits = scan(dir.path(), &home_tilde_rule(), &crate::built_in_walk_prune_names()).unwrap();
    assert!(hits.is_empty());
}
```

- [ ] **Step 2: Confirm they fail**

`cargo test -p vox-arch-check forbidden_patterns` → FAIL (`home_tilde_rule` undefined), `running N tests` with N > 0.

- [ ] **Step 3: Register the rule in TOML**

Append to `docs/src/architecture/layers.toml` after line 539. Use single-quoted TOML literals, matching the file's convention:

```toml
[[forbidden_pattern]]
name             = "no-literal-home-tilde"
exempt_tests     = true
pattern          = '(PathBuf::from|Path::new)\(\s*"~/'
file_glob        = "crates/**/*.rs"
exempt_files     = []
allow_annotation = "// vox-arch-check: allow home-tilde"
reason           = "Rust does not expand `~`; use vox_config::paths::user_home_dir()."
```

- [ ] **Step 4: Verify against the real tree**

```bash
cargo test -p vox-arch-check forbidden_patterns
cargo run -p vox-arch-check --quiet > /tmp/arch.log 2>&1; echo "EXIT=$?"
```

The entry point is `cargo run -p vox-arch-check` — **`vox diag arch-check` does not exist** (`vox diag` exposes only `doctor`).

Expected: `EXIT=0`. The anchored pattern has **zero violations** on today's tree (both raw matches are in `#[cfg(test)]` blocks, which `exempt_tests` covers). **Stop condition:** if any violation appears, fix or annotate it in this same commit — the global `error` guard means a rule with open violations breaks CI for every contributor.

- [ ] **Step 5: Commit**

```bash
git add docs/src/architecture/layers.toml crates/vox-arch-check/src/forbidden_patterns.rs
git commit -m "Gate literal ~ paths in path constructors"
```

---

### Task 10: Repair the dead `cfg(feature)` gates

Four `#[cfg(feature = "…")]` gates reference features that do not exist, so their code is unreachable. `Cargo.toml:42` sets `unexpected_cfgs = "warn"`, so nothing fails.

The macOS-relevant one: `crates/vox-ml-cli/src/commands/schola/train/run_train.rs:154` gates on `not(feature = "mens-candle-metal")` — **vox-ml-cli has no such feature**. The negation is always true, so `vox mens train --device metal` always bails, advising a rebuild with a feature that exists on no crate. **Apple-GPU training is unreachable by construction.**

Also dead: `crates/vox-cli/src/telemetry_spool.rs:147`, `pipeline.rs:125`, `commands/extras/ars/registry.rs:34` all gate on `feature = "vox-gamify"`, but `vox-gamify` is referenced only as `dep:vox-gamify`, so no implicit feature exists.

**Files:**
- Modify: the four sites above; `crates/vox-ml-cli/Cargo.toml`; `Cargo.toml:42`

- [ ] **Step 1: Confirm the features do not exist**

```bash
grep -nE '^\s*(mens-candle-metal|vox-gamify)\s*=' crates/vox-ml-cli/Cargo.toml crates/vox-cli/Cargo.toml
```

Expected: no output for either name as a `[features]` key.

- [ ] **Step 2: Fix each site**

Either declare the missing feature (for `mens-candle-metal`, add it to vox-ml-cli's `[features]` and wire the Metal path), or delete the dead gate. For the three `vox-gamify` sites, the correct gate is `#[cfg(feature = "gamify")]` if such a feature exists, else remove. Correct the two remediation strings that say `-p vox-cli` for features that live on vox-ml-cli.

- [ ] **Step 3: Turn the warning into a gate**

In `Cargo.toml:42`, change `unexpected_cfgs` from `"warn"` to `"deny"`. This catches the whole class on the existing clippy lane at zero added CI cost — but only after Step 2, or the build breaks.

- [ ] **Step 4: Verify and commit**

```bash
cargo clippy --workspace --all-targets > /tmp/clippy.log 2>&1; echo "EXIT=$?"
git add Cargo.toml crates/vox-ml-cli crates/vox-cli
git commit -m "Repair dead cfg(feature) gates and deny unexpected_cfgs"
```

---

### Task 11: Extend existing CI lanes instead of adding a macOS one

**Scope corrected.** The first draft proposed a new `macos-cold-install.yml` with a headless Axis launch. Both were wrong:

- **The launch step was unsound in four ways.** `ps -p $PID` matches zombies, so it can report success for an already-dead process — a false pass on precisely the abort it exists to catch. `kill $PID` under `set -e` fails the step if the process exited on its own. `vox-gui` pulls `cpal` and registers mic state at startup, so an unbundled Mach-O with no `Info.plist` gets TCC-killed on a hosted runner. And it ran `cargo test -p vox-gui` *before* the pnpm build, but `tauri-build` embeds the gitignored `ui/dist` at compile time, so it could not even compile.
- **The cost was untenable.** Three `cargo install --path` calls do not share `target/`; that is 3+ cold compiles of a 136-crate graph on a 3-core runner at the 10× macOS multiplier — ~1,200–2,400 billable minutes per PR push, with no `timeout-minutes`.
- **Two lanes already do this work.** `setup-e2e.yml` is a clean-room cold-install matrix including `macos-latest`, nightly + `push:main`, that even strips the preinstalled toolchain. `ci.yml:1459` runs `gui_relaunch_smoke` — display-free by construction, per its own header — but only on `[self-hosted, linux, x64]`.

**Files:**
- Modify: `.github/workflows/setup-e2e.yml` (macOS leg)
- Modify: `.github/workflows/ci.yml:1459` (`gui-orchestrator-relaunch-smoke`)
- Modify: `.github/workflows/os-compat-report.yml`

- [ ] **Step 1: Add the mesh install to setup-e2e's macOS leg**

It already has a cargo toolchain and a clean `HOME`, so this adds no runner minutes:

```yaml
      - name: Mesh CLI installs and answers
        if: runner.os == 'macOS'
        shell: bash
        run: |
          set -euo pipefail
          cargo install --path crates/vox-ml-cli --features populi --locked --debug
          vox-ml-cli --help | grep -q populi
          vox populi --help
```

- [ ] **Step 2: Add a macOS leg to the display-free GUI smoke**

At `ci.yml:1459`, change `runs-on` to a matrix over the existing `[self-hosted, linux, x64]` **and** `macos-latest`. Keep `VOX_GUI_RELAUNCH_SMOKE=1` and `VOX_ORCHESTRATOR_D_BIN`. This is the startup-wiring coverage the deleted launch step was reaching for, without a window server.

- [ ] **Step 3: Add a macOS leg to the portability report**

In `os-compat-report.yml`, matrix over the **runner label**, not an OS string — a naive `os:` matrix moves the Linux leg off the self-hosted fleet, contradicting the local-first policy:

```yaml
    strategy:
      fail-fast: false
      matrix:
        runner: [[self-hosted, linux, x64], macos-latest]
    runs-on: ${{ matrix.runner }}
```

Suffix the `upload-artifact` name with the runner or the two legs collide on the same artifact name and the second fails. Commit only the Linux leg's output.

- [ ] **Step 4: Verify all three parse**

```bash
for f in setup-e2e ci os-compat-report; do
  uv run --no-project --with pyyaml python -c "import yaml;yaml.safe_load(open('.github/workflows/$f.yml'));print('$f ok')"
done
act --list --workflows .github/workflows/setup-e2e.yml
```

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/setup-e2e.yml .github/workflows/ci.yml .github/workflows/os-compat-report.yml
git commit -m "Cover macOS in the existing e2e, GUI smoke, and portability lanes"
```

---

### Task 12: Sign and notarize the macOS release artifacts

**Scope corrected by measurement.** The first draft assumed `curl | sh` was quarantined. It is not — verified against the real published artifact:

| Check | Result |
|---|---|
| quarantine xattr after `curl` | **absent** (`curl` does not set it; LaunchServices does, for browsers) |
| `codesign -dv` | ad-hoc, linker-signed |
| `./voxup --version` | **exit 0** |
| same binary + quarantine xattr | **SIGKILL, exit 137** |

So **no `xattr -d` belongs in the installer** — it would be cargo-cult. The real break is the **browser** path: a tarball downloaded from the Releases page carries the xattr through extraction, and since nothing signs or notarizes, the binary is killed silently (or blocked with a Gatekeeper dialog when launched from Finder). `docs/superpowers/specs/2026-08-20-vox-distribution-system-design.md:86` already admits this.

There is zero signing in `release-binaries.yml`. The only Apple signing anywhere is `release-gui.yml:130-136` (GUI only). `contracts/manifest/vox-bundle.v1.schema.json:61-62` declares `macos_team_id_env` / `macos_notarize` fields that **nothing reads** — dead schema surface.

- [ ] **Step 1: Confirm the current state**

```bash
gh release download v0.6.0-rc.4748 -p 'voxup-*-aarch64-apple-darwin.tar.gz' -D /tmp/q
tar -xzf /tmp/q/voxup-*.tar.gz -C /tmp/q
xattr /tmp/q/voxup; codesign -dv /tmp/q/voxup 2>&1 | head -3; spctl -a -t install /tmp/q/voxup
```

Expected: no quarantine, ad-hoc signature, `spctl` **rejected**.

- [ ] **Step 2: Sign and notarize in the release lane**

In `release-binaries.yml`, for the two `*-apple-darwin` slices, add Developer ID signing with hardened runtime, then `notarytool submit --wait`, reusing the `APPLE_*` secrets already wired for the GUI lane. **Entitlement note:** hardened-runtime *library validation* refuses to `dlopen` a plugin `.dylib` not signed by the same Team ID — which is exactly what `vox-plugin-host`'s loader does. Add `com.apple.security.cs.disable-library-validation`, or plugins break in the notarized build.

- [ ] **Step 3: Gate it**

Add a `codesign --verify --strict` + `spctl -a` assertion so an unsigned regression cannot ship. Until signing lands, add a note to the Releases page and `installation.md`: install via `curl`, not the browser.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release-binaries.yml
git commit -m "Sign and notarize the macOS release binaries"
```

---

### Task 13: Collapse the installation docs to one source of truth

Seven install paths exist and contradict each other.

| Path | macOS | State |
|---|---|---|
| `curl … \| sh` (voxup) | Yes | **Works** — canonical |
| `cargo install --path crates/vox-cli` | Yes | Works |
| `.msi` | No | **Broken** — `release-installers.yml:76` admits the binary is never built |
| `.deb` | No | Built, **never uploaded** |
| Homebrew | Nominally | **`echo "Simulating Homebrew Tap update..."`** — no formula, no tap |
| `install.sh --dev` / `plan` | — | **Flags do not exist**; `main()` ignores `$@` |

Worst contradictions: `docs/src/reference/ref-installation.md:20,42,44-54` documents an entirely fictional CLI. `crates/voxup/README.md:9,14` advertise `voxlang.org/install` and `/install.ps1` — both **404**. `README.md:60` calls Homebrew/MSI/DEB "configured in the CI pipeline." Doctor's update hint tells macOS users `sudo apt install` or `winget upgrade`.

- [ ] **Step 1: Fix the root cause of the `/releases/latest` 404**

Every tag is `v*-rc.N`, and the release action auto-marks semver-prereleases, which is why `/releases/latest` 404s. **Cut one non-prerelease `vX.Y.Z` tag.** That restores the endpoint and lets both installers drop the list-and-sort logic entirely — retiring the tag-ordering assumption (`/releases` sorts by `created_at`, not `published_at`) and the draft-visibility edge case in one move. The current parse is a workaround for a release-process problem.

- [ ] **Step 2: Delete the fictional docs**

Delete `docs/src/reference/ref-installation.md` and redirect it to `installation.md`. Reduce `README.md:58-72` and both crate READMEs to one command plus a link, fixing the `/install` → `/voxup` URLs.

- [ ] **Step 3: Promote the facts into code**

Add consts beside the existing ones in `crates/vox-cli/src/utils/install_policy/mod.rs` for the ml-cli path and its required feature, and have `main.rs:115` render its hint from them. Extend `command-compliance` to assert every documented `cargo install --path` line in `*.md` matches a const. That converts the doc drift from "someone must notice" into a CI failure.

- [ ] **Step 4: Extend the URL-serving test**

`documented_install_urls_are_served` hardcodes the two `voxup` names, which is why the 404ing `/install` URLs slipped through. Scan all Markdown for `voxlang.org/<path>` and assert a matching `docs-astro/public/<path>` or `_redirects` rule. ~15 lines.

- [ ] **Step 5: Implement or delete the stub packaging jobs**

A job whose body is `echo "Simulating…"` while `README.md` claims it is configured is worse than no job.

- [ ] **Step 6: Fix doctor's update hint**

`tail.rs:262-271` suggests `apt`/`winget`/`cargo install vox-cli`. There is no apt repo, no winget manifest, and `vox-cli` is not on crates.io. For a voxup install the answer is `voxup update`. Task 6's remediation lint will catch this if the hint names a `vox` command.

- [ ] **Step 7: Commit**

```bash
git add README.md docs/ crates/vox-cli/src/utils/install_policy/ crates/voxup/README.md
git commit -m "Collapse installation docs to one source of truth"
```

---

## Deferred

- **`vox run --sandbox` provides zero isolation on macOS.** `crates/vox-cli/src/commands/runtime/run/sandbox.rs:186-199` — Linux gets Landlock, Windows gets Job Objects, macOS prints a warning and returns `Ok(())`. The command *succeeds*, so an operator believes they are sandboxed. Needs a `sandbox-exec` profile or a hard error; security-relevant, own plan.
- **`vox-mesh-policy` and `vox-scaling-policy` are near-identical copies** that both had to absorb the same six fields. Deduping removes one of three places a future field must be added.
- **Live `act` run.** Only ever dry-run. Measurement, not implementation — safe to defer.
- **`cargo-hack` feature powerset sweep** on a weekly non-blocking lane, as a general successor to Task 0's enumerated list. Use `--feature-powerset --depth 1` (not `--each-feature`, which implies `--no-default-features` and does not model `cargo install --features X`).

## Self-Review

**Audit coverage.** Every finding from the seven tracks is either a task above or an explicit Deferred entry. Corrections folded in: Task 9 lost a redundant rule, changed file type (TOML, not Rust), changed regex (anchored, not bare), and fixed its verification command; Task 11 dissolved a new workflow into three existing lanes and deleted an unsound launch assertion; Task 12 was rewritten from an assumption to a measurement; Tasks 6–7 replaced source-grep tests with pure classifiers; Task 1 gained the step that actually edits the hint, and `accepts_inference_workloads` flipped to `false`; Task 0 was created because one missing gate explains three defects.

**Line numbers verified 2026-09-04** against the tree at `f1723b1`: `main.rs:27` (Populi variant), `main.rs:115` (hint), `populi_lifecycle.rs:110` (insertion point), `envelope.rs:47,55,168`, `layers.toml:99,507,533`, `forbidden_patterns.rs:97` (`scan`), `Cargo.toml:282` (`which`). The first draft's `tail.rs:305-325` had drifted ~35 lines and `forbidden_patterns.rs:418-431` named a test fixture, not the struct — both corrected.

**Known risks.** Task 3's `install_from_path` signature is assumed; adapt the call, not the assertion. Task 6's `--probe` change touches a contract-tested Dockerfile healthcheck. Task 12 needs Apple credentials this plan cannot verify. Task 5 stays blocked on an owner decision by design.
