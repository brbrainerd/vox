# P1 — Plugin Capability Layer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. Steps use `- [ ]` checkboxes.
>
> **Read [`2026-09-05-00-INDEX.md`](2026-09-05-00-INDEX.md) first** for the file-ownership rules and global constraints.

**Goal:** make the plugin system the capability-resolution layer every package manager delegates to, so a GPU backend can actually be installed and loaded by a user who has never cloned the repo.

**Architecture:** package managers ship one lean core per triple; hardware-dependent code is resolved *after* install, on the target machine, because that is the only place the hardware is visible. No package manager can express "install the GPU build if this machine has a GPU" (spec §1), but all six can express a package name — so audience is a package identity and capability is a plugin.

**Spec:** [`../specs/2026-09-04-distribution-and-plugin-architecture.md`](../specs/2026-09-04-distribution-and-plugin-architecture.md) §2, §4

**You own:** `crates/vox-plugin-*/`, `crates/vox-plugin-host/`, `crates/vox-plugin-catalog/`, `crates/vox-ml-cli/src/commands/mens/`, `crates/vox-ml-cli/src/commands/schola/`

## Global constraints

See the index. The two that bite hardest here:

- **No runtime `cargo`.** An installed user has no toolchain and no checkout.
- **Never execute a downloaded binary** while verifying, and never set `com.apple.quarantine`.

## Why this is the critical path

Measured on `main`, and **broader than the first draft of this plan claimed**:

- **All 12** `github:vox-foundation/vox-plugin-*` sources 404 — not just CUDA. That
  includes `oratio`, `populi-mesh`, `browser`, and seven `skill-*` plugins.
- **Six** sources are `local:<repo-relative>` and cannot resolve off a clone:
  `nvml-probe` (:20), `mens-candle-metal` (:40), `webhook` (:154), `runtime-wasm`
  (:172), `runtime-container` (:181), `publication` (:190).
- **The URL is malformed regardless.** `install.rs:232` hardcodes
  `let version = "latest"`, emitting
  `…/releases/latest/download/{id}-latest-{triple}.zip`. An earlier draft of this
  plan said the resolver already produces `…/download/v{version}/{id}-v{version}-{triple}.zip`
  and concluded that repointing was "sufficient". **That was wrong** — repointing
  at a valid repo still 404s, and P4 would be asked to publish assets under a name
  nothing ever requests.
- **`local:` is not gated.** An earlier draft said it was "deliberately gated behind
  `VOX_LOCAL_PLUGIN_FALLBACK`". **No such variable exists.** The real one is
  `VOX_NO_LOCAL_PLUGIN_FALLBACK` (opt-**out**) and it guards only the `github:`
  branch at `:219`; the `local:` branch at `:226` is ungated. Metal is not "refused
  by design" — it is an unreachable-path bug.

So the whole catalog is unreachable for an installed user. Everything else in the
spec assumes this layer works.

**Read `install.rs:213-241` before starting Task 2.**

---

## Task 1: Make the capability tags load-bearing

`requires-tag = "nvidia-gpu"` / `"apple-silicon"` already exist in `catalog.toml`
(`:19, :29, :39`). **No *resolver* consumes them** — the only reader is
`crates/vox-cli/src/commands/plugin/info.rs:22`, which prints the value. (An earlier
draft said "nothing reads them"; that absolute phrasing would stall an executor who
greps and finds the hit.) Making the tags decide *which plugin loads* is the piece
no package manager can do.

**Files:**
- Create: `crates/vox-plugin-host/src/capability.rs`
- Modify: `crates/vox-plugin-host/src/lib.rs`
- Test: `crates/vox-plugin-host/src/capability.rs` (inline `mod tests`)

**Interfaces:**
- Produces: `pub fn probe() -> CapabilitySet`, `pub struct CapabilitySet(BTreeSet<String>)`, `pub fn satisfies(&self, requires_tag: Option<&str>) -> bool`
- Consumed by: Task 3, Task 4

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn a_plugin_with_no_requires_tag_is_always_satisfied() {
    let caps = CapabilitySet::from_tags(["cpu-only"]);
    assert!(caps.satisfies(None));
}

#[test]
fn a_requires_tag_must_be_present_in_the_probe() {
    let caps = CapabilitySet::from_tags(["apple-silicon", "metal"]);
    assert!(caps.satisfies(Some("apple-silicon")));
    assert!(!caps.satisfies(Some("nvidia-gpu")));
}
```

- [ ] **Step 2: Run it and confirm it fails**

`cargo test -p vox-plugin-host --all-targets capability > /tmp/p1t1.log 2>&1; echo $?`
Expected: FAIL, `cannot find type CapabilitySet`.

- [ ] **Step 3: Implement `CapabilitySet` and `probe()`**

`probe()` emits tags from the host: `cpu-only` always; `apple-silicon` on `target_arch = "aarch64"` + `target_os = "macos"`; `metal` when a Metal device enumerates; `nvidia-gpu` and `cuda-<major>` when the CUDA driver library loads. Probing must **never panic and never require a toolchain** — a failed probe yields fewer tags, not an error.

- [ ] **Step 4: Run tests, confirm pass**

- [ ] **Step 5: Add a test proving the probe is total**

```rust
#[test]
fn probe_never_panics_and_always_reports_cpu_only() {
    let caps = probe();
    assert!(caps.satisfies(Some("cpu-only")));
}
```

- [ ] **Step 6: Commit** — `feat(plugin-host): probe host capabilities and honor requires-tag`

---

## Task 2: Repoint both GPU plugins at reachable, checksummed artifacts

**Files:**
- Modify: `crates/vox-plugin-catalog/catalog.toml`
- Test: `crates/vox-plugin-catalog/tests/sources_are_reachable.rs` (create)

No new source *kind* is needed — but the existing resolver cannot be reused as-is
(see "Why this is the critical path"). Step 0 extracts and tests URL construction
first; only then is repointing meaningful.

- [ ] **Step 0: Extract and test URL construction first.** This is the blocking
      correction. Create `crates/vox-plugin-catalog/src/artifact.rs`:
      `pub fn release_asset_url(gh: &str, id: &str, version: &str, triple: &str) -> String`.
      Assert the exact literal, and assert it contains no `"latest"`:

```rust
#[test]
fn release_asset_url_interpolates_the_real_version() {
    let u = release_asset_url("vox-foundation/vox", "mens-candle-metal", "0.6.0", "macos-aarch64");
    assert_eq!(u, "https://github.com/vox-foundation/vox/releases/download/v0.6.0/mens-candle-metal-v0.6.0-macos-aarch64.zip");
    assert!(!u.contains("latest"), "resolver must not emit the literal `latest`");
}
```

- [ ] **Step 1: Write the failing test** — assert every `[[plugin]]` with
      `payload-kind = "code"` resolves to a URL via `release_asset_url`.
      **Do not** assert "no `local:` sources" — there are **six**, and only one is in
      this task's scope, so that assertion stays red after the task is complete.
      **Do not** assert "the repo exists": `github:vox-foundation/vox` obviously
      exists, so it passes on a URL that 404s at download — the exact defect being
      fixed. It is also a network call inside a unit test, which contradicts the
      hermeticity gate P4 is adding.

- [ ] **Step 2: Run it, confirm it fails.**
      Run: `cargo test -p vox-plugin-catalog --all-targets artifact > /tmp/p1t2.log 2>&1; echo $?`
      Expected: FAIL — `release_asset_url` not found.

- [ ] **Step 3: Repoint the two GPU plugins** to `github:vox-foundation/vox`.
      **Land the flip behind a fail-closed message naming the missing asset**, and do
      not switch it live until P4 confirms the asset publishes — otherwise this
      converts one 404 into two and removes the only signal that made the Metal case
      legible. Record the `HEAD` status code when you flip it.

- [ ] **Step 4: Run, confirm pass.**
      Run: `cargo test -p vox-plugin-catalog --all-targets > /tmp/p1t2.log 2>&1; echo $?`
      then `grep -E '^test result:' /tmp/p1t2.log` — fail if absent or `0 passed`.

- [ ] **Step 5: Commit** — `fix(catalog): point GPU plugins at reachable release assets`

**Cross-plan request:** the release workflow must publish `mens-candle-{cuda,metal}-v{version}-{triple}.zip` and include them in `checksums.txt`. **P4 owns `.github/workflows/`** — record this in P4's inbox; do not edit workflows here.

---

## Task 3: Select the ML backend by capability, not by string literal

Every `MlBackend` call site names `"mens-candle-cuda"` literally, so **nothing can ever load Metal** — the only GPU plugin that ships (spec §4.1 P2).

**Files:**
- Modify: `crates/vox-plugin-host/src/lib.rs`
- Modify: `crates/vox-ml-cli/src/commands/schola/merge_qlora.rs` (line ~105)
- Modify: `crates/vox-ml-cli/src/commands/schola/train/run_train.rs`

**Interfaces:**
- Consumes: `CapabilitySet` from Task 1
- Produces: `pub fn resolve_extension_point(ep: &str) -> Result<CachedPlugin>`

- [ ] **Step 1: Write the failing test** — with a fake catalog offering both backends and a probe reporting only `apple-silicon`, resolving `MlBackend` returns `mens-candle-metal`; with only `nvidia-gpu`, it returns `mens-candle-cuda`; with neither, it returns a *diagnosable* error naming the tags that were missing.

- [ ] **Step 2: Run, confirm fail.**

- [ ] **Step 3: Implement `resolve_extension_point`** and replace the literals at both call sites.

- [ ] **Step 4: Run, confirm pass.**

- [ ] **Step 5: Fix the remediation strings.** `run_train.rs:140` currently says `cargo build -p vox-ml-cli --features gpu,mens-candle-cuda`. An installed user has no checkout. Replace with `vox plugin install mens-candle-metal` (or whichever the probe indicates), and verify the command exists in the clap tree.

- [ ] **Step 6: Commit** — `fix(ml): select the ML backend by capability instead of a hardcoded id`

---

## Task 4: Remove the runtime compiler dependency

`plugin_heal` shells out to `cargo build` at runtime (spec §4.1 P6), and `plugin/install.rs:112` tells users to run `cargo build -p <crate> --release`. Both assume a toolchain and the repo.

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/mens/plugin_heal.rs`
- Test: `crates/vox-ml-cli/tests/no_runtime_cargo.rs` (create)

- [ ] **Step 1: Write the failing test** — assert no runtime `cargo` invocation.
      A bare grep for `Command::new("cargo")` is **defeatable** (`Command::new(cargo_bin())`,
      `env cargo`, a `const CARGO: &str`), so also assert behaviourally: run the heal
      path with `PATH` set to an empty directory and assert it still succeeds.
      **Scope note:** this lint covers `vox-ml-cli` only. There are ~9 more runtime-`cargo`
      sites in `vox-cli` (`run.rs:209`, `test.rs:54`, `gui.rs:10,82,119`,
      `checks_codex.rs:19,49`, `tail.rs:78`, `test_health.rs:14`) which **no plan owns**.
      File a cross-plan request rather than reaching into them.

- [ ] **Step 2: Run, confirm fail.**

- [ ] **Step 3: Replace the rebuild path with a verified artifact fetch** — the same path Task 2 established.

- [ ] **Step 4: Run, confirm pass.**

- [ ] **Step 5: Commit** — `fix(plugins): heal by fetching a verified artifact, never by compiling`

---

## Task 5: Pin plugin version to core version, and verify signatures

Spec §4.2(d) and §4.1 P8. `installed_version()` currently returns readdir order (P7).

**Files:**
- Modify: `crates/vox-plugin-host/src/lib.rs`
- Test: inline

- [ ] **Step 1: Write failing tests** — (a) a plugin whose manifest version differs from the running core version is refused with a version-mismatch error, not loaded; (b) `installed_version()` is deterministic across two calls with several versions present.

- [ ] **Step 2: Run, confirm fail.**

- [ ] **Step 3: Implement.** The artifact name already encodes the version (`{id}-v{version}-{triple}.zip`), so the check is a comparison, not new metadata.

- [ ] **Step 4: Verify the dylib signature before `dlopen`.** This layer is now the primary delivery path for native code, so the deferred item becomes mandatory. Verify the archive checksum against `checksums.txt` before extraction, and refuse to load on mismatch.

- [ ] **Step 5: Run, confirm pass.**

- [ ] **Step 6: Commit** — `feat(plugin-host): gate loading on version match and verified checksum`

---

## Task 6: Make the tier system real, or delete it

`voxup/src/install.rs` reads `tier` only to log it — `full` never fetches `vox-ml-cli`, no tier ships a GUI, and `minimal`/`default` have identical binary lists (spec §4.1 P5). The parity test validates the YAML against itself and never against installer behaviour.

**Cross-plan note:** `crates/voxup/src/` is owned by **P5** and `contracts/distribution/profiles.v1.yaml` by **P7**. Do not edit either.

- [ ] **Step 1:** Record in P5's and P7's inboxes: the tier declaration must either drive `install.rs` or be deleted, and `distribution_parity.rs` must assert installer behaviour rather than YAML self-consistency.

- [ ] **Step 2:** Provide the interface those plans need: a `resolve_bundle(tier) -> Vec<PluginId>` in `vox-plugin-catalog`, tested here, so P5 can call it without duplicating catalog logic.

- [ ] **Step 3: Commit** — `feat(catalog): expose bundle resolution for the installer`

---

## Verification

- [ ] `cargo test -p vox-plugin-host -p vox-plugin-catalog -p vox-ml-cli --all-targets > /tmp/p1.log 2>&1; echo $?` — record the real pass/fail counts, not just the exit code.
- [ ] `cargo run -p vox-arch-check --bins` — layer rules still hold. *(Note: this reports "0 passed" without `--bins`; tests live in a bin target.)*
- [ ] Confirm no `Command::new("cargo")` outside tests in any owned crate.
- [ ] Confirm no remediation string in an owned crate references a repo-relative path.

## Cross-plan requests

| To | Request |
|---|---|
| P4 | Publish `mens-candle-{cuda,metal}-v{version}-{triple}.zip` as release assets and include them in `checksums.txt` |
| P5 | Make `install.rs` honour the tier, using `resolve_bundle` from Task 6 |
| P7 | Decide whether `profiles.v1.yaml` tiers survive; if so, reconcile them with `catalog.toml` bundles — today the two taxonomies share no ids and no code path |
| P7 (general `vox-cli/` ownership) | **RESOLVED** on branch `claude/vox-cli-cargo-cleanup-470499`, commit `9a2127fc6`. Surveyed all 11 sites named below: `gui.rs:10,77` were already gated by earlier work (a `debug_assertions`-only branch and a workspace-detection gate with an honest no-checkout message); `checks_codex.rs:19,49`, `checks_standard/toolchain.rs:20`, `checks_standard/test_health.rs:14,40,58`, and `graphify/mod.rs:658,667` are all contributor-only, workspace-scoped diagnostic/maintenance tools (`cargo tree -p vox-cli`, `cargo test --workspace --no-run`, `cargo xtask dep-budget`, a rebuild-fingerprint tracer) that gracefully report absence, no crash, no bad remediation string — confirmed fine as-is, no installed user reaches them meaningfully. Only `commands/test.rs:54` (`vox test`, architecturally identical to `vox run`'s already-fixed cargo dependency) needed a fix; added the same honest preflight `vox run` already has. |

## Cross-plan inbox

Rows filed here by other plans. Each must be executable with no conversation.

| From | Request |
|---|---|
| P7 | Tier ↔ bundle taxonomy is DECIDED — see INDEX §2.1. Both taxonomies are kept as orthogonal axes; P7 adds a `bundle:` key to each tier in `contracts/distribution/profiles.v1.yaml`, mapping `minimal→vox-base`, `default→vox-fullstack`, `full→vox-dev`. No new bundle ids; P7 does NOT edit `catalog.toml` (yours). P1 Task 6 implements against INDEX §2.1 and must keep using the existing `bundle_resolved()` at `crates/vox-plugin-catalog/src/lib.rs:72` — no third spelling. If you rename or remove any of those three bundle ids, tell P7: a test in `crates/vox-cli/tests/` asserts every tier's `bundle` resolves. |
