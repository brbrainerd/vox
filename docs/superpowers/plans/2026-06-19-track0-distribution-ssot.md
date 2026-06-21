# Track 0: Distribution SSOT Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create one authoritative distribution manifest (`contracts/distribution/profiles.v1.yaml`) describing install tiers, their dependency closures, the crates.io publish set, and the released binary set — plus a CI parity gate that fails when the manifest drifts from reality. This is the prerequisite for Tracks A–D.

**Architecture:** A declarative YAML SSOT lives under `contracts/distribution/`. A small typed reader in the `voxup` crate (`profiles.rs`) parses it. A Rust integration test (`tests/distribution_parity.rs`) asserts the manifest is internally consistent and matches on-disk facts (the toolchain contract, `crates/_public.toml`, `layers.toml` `publishable=true`, and the active workspace binaries). The test is wired into CI as a required check. We follow the existing parity-test pattern in `crates/vox-telemetry/tests/taxonomy_ssot_parity.rs`.

**Tech Stack:** Rust, `serde` + `serde_yaml`, `toml` (already a workspace dep), standard `cargo test`. No new crate, no new `.ps1`/`.sh`/`.py` (AGENTS.md VoxScript-only rule). `cargo fmt -p voxup` only (never `cargo fmt --all`).

---

## Critique pass — codebase audit (2026-06-19)

This plan was hardened against the live codebase. Fixes already folded in:

- **Path bug fixed.** `crates/_public.toml` is under `crates/`, so the test path
  is `../_public.toml` (Task 6), not `../../_public.toml` (which would point at a
  non-existent repo-root file).
- **Dependency reality.** `voxup` already has `serde` (derive) + `serde_yaml`
  (the maintained `serde_yaml_ng` 0.10 fork via package-rename). Only `toml` is
  added (Task 2). The deprecated `serde_yaml = "0.9"` fallback was removed.
- **workspace-hack + lock.** Adding a dep can change hakari output and always
  changes `Cargo.lock`; CI runs `--locked`, so Task 2 now regenerates the hack
  and commits `Cargo.lock`.
- **Toolchain check tightened.** The contract key is `versions.rust` — Task 5
  parses it exactly instead of a loose substring match.
- **`publish = false` drift surfaced.** `voxup` carries `publish = false` while
  being in the publish set — fine while `publish.enabled: false`, but a trap at
  flip time. Task 6 adds a forward-looking readiness gate.
- **Cross-reference.** Track C (publish) must reconcile with the existing
  crates.io program (`project_gamify_gui_pluginization_plan_2026_06_18`):
  its **TrackB hakari-aware publish** (workspace-hack blocks all closures →
  publish the hack first) and **R18 publishability arch-check gate**. Do not
  build a second, conflicting publish mechanism — the SSOT here is the *data*;
  that program owns the *publish machinery*.

---

## Gemini-Flash Execution Preamble

- Work only inside the files named per task. Do **not** restructure unrelated code.
- After each code change to `voxup`, format with `cargo fmt -p voxup` (the workspace-wide `cargo fmt --all` is banned).
- Run tests with `cargo test -p voxup`. Do **not** pipe `cargo` to `head`/`grep` on Windows (it orphans processes) — redirect to a file if you must capture output: `cargo test -p voxup > test-out.txt 2>&1`.
- Commit after every task with the exact message given.
- `[SEQUENTIAL]` tasks must be done in order. `[PARALLEL-SAFE]` tasks touch disjoint files and may be reordered.
- If a referenced on-disk fact (a path, a crate name) does not exist, STOP and report — do not invent it.

---

## File Structure

| File | Responsibility | Action |
|------|----------------|--------|
| `contracts/distribution/profiles.v1.yaml` | The SSOT: tiers, dep closures, publish set, binary set. | Create |
| `crates/voxup/src/profiles.rs` | Typed reader + structs for the SSOT. | Create |
| `crates/voxup/src/lib.rs` | Expose `profiles` module (create lib if absent). | Create/Modify |
| `crates/voxup/Cargo.toml` | Add `serde_yaml` dep; ensure `serde` with derive; declare lib target. | Modify |
| `crates/voxup/tests/distribution_parity.rs` | Integration test: internal + on-disk parity. | Create |
| `.github/workflows/distribution-parity.yml` | Hosted required check running the parity test. | Create |

---

## Task 1: Author the distribution SSOT YAML `[SEQUENTIAL]`

**Files:**
- Create: `contracts/distribution/profiles.v1.yaml`

- [ ] **Step 1: Create the manifest**

Create `contracts/distribution/profiles.v1.yaml` with exactly this content:

```yaml
# Distribution SSOT (Track 0). Single source of truth for install tiers,
# their dependency closures, the crates.io publish set, and released binaries.
# Drift from on-disk reality is caught by crates/voxup/tests/distribution_parity.rs.
schema_version: 1

# Rust toolchain version. MUST equal the version in
# contracts/toolchain/workspace-toolchain.v1.yaml (parity test enforces this).
rust_version: "1.96.0"

# Binaries produced by release + nightly. vox-bootstrap and vox-schola are RETIRED.
binaries:
  - vox
  - vox-ml-cli
  - voxup

# Install tiers. Each tier declares the binaries it ships, its build-time deps
# (only relevant on the from-source path), and its runtime-optional deps
# (detected/enabled by `vox doctor`, never required to install).
tiers:
  minimal:
    description: "Compiler / language CLI only."
    binaries: [vox]
    build_deps: [rust]
    runtime_optional: []
  default:
    description: "CLI plus desktop GUI."
    binaries: [vox]
    build_deps: [rust, node, tauri-system-libs]
    runtime_optional: []
  full:
    description: "Everything: CLI, GUI, ML, agy delegation, curated plugins."
    binaries: [vox, vox-ml-cli, voxup]
    build_deps: [rust, node, tauri-system-libs]
    runtime_optional: [agy, model-weights, plugins]

# crates.io publish set. MUST be a superset-consistent reconciliation of
# crates/_public.toml. Order is leaf-first (publish order). `enabled: false`
# means the public flip is deferred (readiness only).
publish:
  enabled: false
  crates:
    - vox-crypto
    - vox-plugin-types
    - vox-plugin-api
    - vox-plugin-sdk
    - voxup

# Crates that MUST NOT be published or appear on the install/build path as a
# hard dependency carrier for `agy`. The parity test asserts none of these are
# in `publish.crates`.
non_publishable:
  - vox-orchestrator-mcp
```

- [ ] **Step 2: Verify it parses as YAML**

Run: `cargo run -p voxup -- --help` is NOT needed here. Instead validate via Python-free check — open the file and confirm structure visually, then rely on Task 4's test. No command in this step.

- [ ] **Step 3: Commit**

```bash
git add contracts/distribution/profiles.v1.yaml
git commit -m "feat(dist): distribution SSOT manifest (tiers, deps, publish set, binaries)"
```

---

## Task 2: Add deps + lib target to `voxup` `[SEQUENTIAL]`

**Files:**
- Modify: `crates/voxup/Cargo.toml`

> **Audited reality (2026-06-19):** `crates/voxup/Cargo.toml` ALREADY has
> `serde = { workspace = true, features = ["derive"] }` and
> `serde_yaml = { workspace = true }` (the workspace `serde_yaml` is the
> maintained `serde_yaml_ng` 0.10 fork via package-rename — `serde_yaml::` paths
> still work). It does **NOT** have `toml`. `voxup` has no `[lib]`/`src/lib.rs`
> (binary-only: `main.rs` + sibling modules). Do not add `serde_yaml = "0.9"` —
> 0.9 is RUSTSEC-deprecated and banned by the workspace.

- [ ] **Step 1: Add the one missing dependency**

In `crates/voxup/Cargo.toml`, under `[dependencies]`, add exactly one line
(keep alphabetical-ish grouping; `serde`/`serde_yaml` are already present — do
not touch them):

```toml
toml = { workspace = true }
```

- [ ] **Step 2: Add a lib target alongside the existing binary**

`voxup` is binary-only today. Add a `[lib]` section so the parity test can link
against `voxup::profiles`. The binary (`main.rs`) and the lib coexist; `main.rs`
keeps its own `mod` declarations and is unaffected. Add to `crates/voxup/Cargo.toml`:

```toml
[lib]
name = "voxup"
path = "src/lib.rs"
```

- [ ] **Step 3: Regenerate workspace-hack + lock (prevents CI `--locked` failure)**

`voxup` depends on `workspace-hack`. Adding a dep can change the hakari output and
will change `Cargo.lock`; the CI gate (Task 8) runs `--locked`, so both must be committed.

Run: `cargo hakari generate` (if `cargo-hakari` is installed; if the command is
absent, skip — `toml` is already a widely-used workspace dep so the hack is
unlikely to change). Then refresh the lock: `cargo build -p voxup > build-out.txt 2>&1`.
Expected: exit 0 (lib.rs is created in Task 3; an "unresolved lib" error here is fine — return after Task 3).

- [ ] **Step 4: Commit (include Cargo.lock and any hakari changes)**

```bash
git add crates/voxup/Cargo.toml Cargo.lock
git add crates/workspace-hack/Cargo.toml 2>/dev/null || true
git commit -m "build(voxup): add toml dep + lib target for distribution SSOT reader"
```

---

## Task 3: Typed SSOT reader `[SEQUENTIAL]`

**Files:**
- Create: `crates/voxup/src/profiles.rs`
- Modify/Create: `crates/voxup/src/lib.rs`

- [ ] **Step 1: Write the failing test (inline unit test in profiles.rs)**

Create `crates/voxup/src/profiles.rs`:

```rust
//! Typed reader for the distribution SSOT (`contracts/distribution/profiles.v1.yaml`).

use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct Profiles {
    pub schema_version: u32,
    pub rust_version: String,
    pub binaries: Vec<String>,
    pub tiers: std::collections::BTreeMap<String, Tier>,
    pub publish: Publish,
    pub non_publishable: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct Tier {
    pub description: String,
    pub binaries: Vec<String>,
    pub build_deps: Vec<String>,
    pub runtime_optional: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct Publish {
    pub enabled: bool,
    pub crates: Vec<String>,
}

/// Parse the SSOT from a YAML string.
pub fn parse(yaml: &str) -> Result<Profiles, serde_yaml::Error> {
    serde_yaml::from_str(yaml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_manifest() {
        let yaml = r#"
schema_version: 1
rust_version: "1.96.0"
binaries: [vox]
tiers:
  minimal:
    description: "x"
    binaries: [vox]
    build_deps: [rust]
    runtime_optional: []
publish:
  enabled: false
  crates: [voxup]
non_publishable: [vox-orchestrator-mcp]
"#;
        let p = parse(yaml).expect("must parse");
        assert_eq!(p.schema_version, 1);
        assert_eq!(p.rust_version, "1.96.0");
        assert!(p.tiers.contains_key("minimal"));
        assert!(!p.publish.enabled);
    }
}
```

- [ ] **Step 2: Expose the module**

Create or modify `crates/voxup/src/lib.rs` to include:

```rust
pub mod profiles;
```

(If `lib.rs` already exists with other content, append the line; do not remove existing exports.)

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test -p voxup profiles::tests::parses_minimal_manifest > test-out.txt 2>&1`
Expected: PASS (1 passed).

- [ ] **Step 4: Format**

Run: `cargo fmt -p voxup`

- [ ] **Step 5: Commit**

```bash
git add crates/voxup/src/profiles.rs crates/voxup/src/lib.rs
git commit -m "feat(voxup): typed reader for distribution SSOT"
```

---

## Task 4: Internal-consistency parity test `[SEQUENTIAL]`

**Files:**
- Create: `crates/voxup/tests/distribution_parity.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/voxup/tests/distribution_parity.rs`:

```rust
//! Distribution SSOT parity gate (Track 0). Mirrors the pattern in
//! crates/vox-telemetry/tests/taxonomy_ssot_parity.rs.

use voxup::profiles::{self, Profiles};

const SSOT_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../contracts/distribution/profiles.v1.yaml"
);

fn load() -> Profiles {
    let txt = std::fs::read_to_string(SSOT_PATH)
        .expect("contracts/distribution/profiles.v1.yaml must exist");
    profiles::parse(&txt).expect("distribution SSOT must parse")
}

#[test]
fn schema_version_is_one() {
    assert_eq!(load().schema_version, 1);
}

#[test]
fn every_tier_binary_is_a_declared_binary() {
    let p = load();
    for (tier, t) in &p.tiers {
        for b in &t.binaries {
            assert!(
                p.binaries.contains(b),
                "tier '{tier}' ships binary '{b}' not in top-level binaries list"
            );
        }
    }
}

#[test]
fn three_tiers_exist() {
    let p = load();
    for name in ["minimal", "default", "full"] {
        assert!(p.tiers.contains_key(name), "tier '{name}' must be declared");
    }
}

#[test]
fn publish_and_non_publishable_are_disjoint() {
    let p = load();
    for c in &p.non_publishable {
        assert!(
            !p.publish.crates.contains(c),
            "crate '{c}' is in BOTH publish.crates and non_publishable"
        );
    }
}

#[test]
fn agy_only_in_full_tier_runtime_optional() {
    let p = load();
    for (tier, t) in &p.tiers {
        let has_agy = t.runtime_optional.iter().any(|d| d == "agy");
        if tier == "full" {
            assert!(has_agy, "full tier must list agy as runtime_optional");
        } else {
            assert!(!has_agy, "tier '{tier}' must NOT list agy");
        }
    }
}
```

- [ ] **Step 2: Run to verify it passes**

Run: `cargo test -p voxup --test distribution_parity > test-out.txt 2>&1`
Expected: PASS (5 passed). If `agy_only_in_full_tier_runtime_optional` fails, the YAML from Task 1 is wrong — fix the YAML, not the test.

- [ ] **Step 3: Commit**

```bash
git add crates/voxup/tests/distribution_parity.rs
git commit -m "test(dist): internal-consistency parity gate for distribution SSOT"
```

---

## Task 5: On-disk parity — toolchain version `[SEQUENTIAL]`

**Files:**
- Modify: `crates/voxup/tests/distribution_parity.rs`

> **Audited reality (2026-06-19):** the contract is
> `contracts/toolchain/workspace-toolchain.v1.yaml`; the Rust version lives at
> `versions.rust: "1.96.0"`. We parse that exact key (not a loose substring match).

> **Why a lib helper (not inline parse):** integration tests under `tests/`
> compile as a separate crate that can only name the package's **public API** and
> its **dev-dependencies** — NOT its regular dependencies. So a test cannot name
> `serde_yaml::Value` or `toml::Value` directly. All YAML/TOML parsing therefore
> lives in `profiles.rs` and returns plain std types (`String`, `Vec<String>`,
> `bool`). Do not add `serde_yaml`/`toml` as dev-deps to work around this.

- [ ] **Step 1: Add the extraction helper to the reader**

Add to `crates/voxup/src/profiles.rs`:

```rust
/// Extract `versions.rust` from a workspace-toolchain.v1.yaml document.
pub fn toolchain_rust_version(yaml: &str) -> Option<String> {
    let v: serde_yaml::Value = serde_yaml::from_str(yaml).ok()?;
    v["versions"]["rust"].as_str().map(String::from)
}
```

- [ ] **Step 2: Add the failing test (names only std types + the helper)**

Append to `crates/voxup/tests/distribution_parity.rs`:

```rust
#[test]
fn rust_version_matches_toolchain_contract() {
    let p = load();
    let contract_txt = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/toolchain/workspace-toolchain.v1.yaml"
    ))
    .expect("workspace-toolchain.v1.yaml must exist");
    let contract_rust = voxup::profiles::toolchain_rust_version(&contract_txt)
        .expect("workspace-toolchain.v1.yaml must have versions.rust");
    assert_eq!(
        contract_rust, p.rust_version,
        "SSOT rust_version '{}' != toolchain contract versions.rust '{contract_rust}'",
        p.rust_version
    );
}
```

- [ ] **Step 3: Run to verify it passes**

Run: `cargo test -p voxup --test distribution_parity rust_version_matches_toolchain_contract > test-out.txt 2>&1`
Expected: PASS. If FAIL, reconcile: the SSOT `rust_version` and the toolchain contract disagree — set both to the same value (Track 0 standardizes on `1.96.0`).

- [ ] **Step 4: Commit**

```bash
git add crates/voxup/src/profiles.rs crates/voxup/tests/distribution_parity.rs
git commit -m "test(dist): SSOT rust_version parity with toolchain contract"
```

---

## Task 6: On-disk parity — publish set vs `_public.toml` `[SEQUENTIAL]`

**Files:**
- Modify: `crates/voxup/tests/distribution_parity.rs`

> **Audited reality (2026-06-19):** the file is `crates/_public.toml` (a top-level
> `crates = [...]` array). `voxup`'s manifest dir is `crates/voxup`, so the
> correct relative path is `../_public.toml` (one level up → `crates/`), **NOT**
> `../../_public.toml` (that resolves to a non-existent repo-root file). The
> earlier draft of this plan had the wrong path — use `../_public.toml`.

- [ ] **Step 1: Confirm `_public.toml` shape**

Run: `cat crates/_public.toml`. Confirm it has a top-level `crates = [...]` array of strings (currently: `vox-crypto`, `voxup`, `vox-plugin-types`, `vox-plugin-api`, `vox-plugin-sdk`).

- [ ] **Step 2: Add TOML helpers to the reader**

`toml::Value` cannot be named from the integration test (same dev-dep rule as
Task 5), so add these helpers to `crates/voxup/src/profiles.rs`:

```rust
/// Extract the top-level `crates = [...]` string array from `_public.toml`.
pub fn public_toml_crates(toml_text: &str) -> Vec<String> {
    let v: toml::Value = match toml::from_str(toml_text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    v.get("crates")
        .and_then(|c| c.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

/// True iff a Cargo.toml sets `[package] publish = false`.
pub fn cargo_publish_is_false(cargo_toml_text: &str) -> bool {
    let v: toml::Value = match toml::from_str(cargo_toml_text) {
        Ok(v) => v,
        Err(_) => return false,
    };
    matches!(
        v.get("package").and_then(|p| p.get("publish")),
        Some(toml::Value::Boolean(false))
    )
}
```

- [ ] **Step 3: Add the failing test (uses the helper)**

Append to `crates/voxup/tests/distribution_parity.rs`:

```rust
#[test]
fn publish_set_is_subset_of_public_toml() {
    let p = load();
    let public = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../_public.toml"
    ))
    .expect("crates/_public.toml must exist");
    let public_crates = voxup::profiles::public_toml_crates(&public);
    assert!(!public_crates.is_empty(), "_public.toml 'crates' array must be non-empty");
    for c in &p.publish.crates {
        assert!(
            public_crates.contains(c),
            "SSOT publish crate '{c}' is not declared in crates/_public.toml"
        );
    }
}
```

- [ ] **Step 4: Add the forward-looking publish-readiness gate**

`voxup` (and likely others in the publish set) currently carry `publish = false`
in their `Cargo.toml` — correct *now* because `publish.enabled` is `false`
(deferred). But when Track C flips `publish.enabled: true`, every publish crate
MUST have `publish` un-set or `true`, or `cargo publish` silently refuses. This
test enforces that invariant *only* when the flip is on, so it is green today and
guards the future. It reuses the `cargo_publish_is_false` helper from Step 2.
Append to `crates/voxup/tests/distribution_parity.rs`:

```rust
#[test]
fn when_publish_enabled_every_crate_is_actually_publishable() {
    let p = load();
    if !p.publish.enabled {
        return; // deferred — Track C flips this; nothing to enforce yet.
    }
    for c in &p.publish.crates {
        let manifest = std::fs::read_to_string(format!(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../{}/Cargo.toml"),
            c
        ))
        .unwrap_or_else(|_| panic!("publish crate '{c}' has no crates/{c}/Cargo.toml"));
        assert!(
            !voxup::profiles::cargo_publish_is_false(&manifest),
            "publish.enabled is true but crate '{c}' has `publish = false` in its Cargo.toml"
        );
    }
}
```

> Note: this uses `crates/{c}/Cargo.toml` — true for `voxup`, `vox-crypto`, and
> the plugin trio. If a future publish crate lives at a non-`crates/<name>` path,
> Track C must generalize this lookup.

- [ ] **Step 5: Run to verify both publish tests pass**

Run: `cargo test -p voxup --test distribution_parity publish > test-out.txt 2>&1`
Expected: PASS (both `publish_set_is_subset_of_public_toml` and
`when_publish_enabled_every_crate_is_actually_publishable`). The readiness test
short-circuits green while `enabled: false`.

- [ ] **Step 6: Commit**

```bash
git add crates/voxup/src/profiles.rs crates/voxup/tests/distribution_parity.rs
git commit -m "test(dist): publish set ⊆ _public.toml + forward publish-readiness gate"
```

---

## Task 7: On-disk parity — declared binaries exist as workspace members `[SEQUENTIAL]`

**Files:**
- Modify: `crates/voxup/tests/distribution_parity.rs`

- [ ] **Step 1: Add the failing test**

Append to `crates/voxup/tests/distribution_parity.rs`:

```rust
#[test]
fn declared_binaries_have_crate_dirs() {
    // Maps SSOT binary name -> crate directory under crates/.
    // vox is produced by vox-cli; vox-ml-cli and voxup match their dir names.
    let dir_for = |bin: &str| -> &str {
        match bin {
            "vox" => "vox-cli",
            other => other,
        }
    };
    let p = load();
    for bin in &p.binaries {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../").to_string() + dir_for(bin);
        assert!(
            std::path::Path::new(&dir).is_dir(),
            "SSOT binary '{bin}' expects crate dir '{dir}' which does not exist"
        );
    }
}
```

- [ ] **Step 2: Run to verify it passes**

Run: `cargo test -p voxup --test distribution_parity declared_binaries_have_crate_dirs > test-out.txt 2>&1`
Expected: PASS. If FAIL, the binary→dir map is wrong; correct `dir_for`.

- [ ] **Step 3: Run the whole parity suite**

Run: `cargo test -p voxup --test distribution_parity > test-out.txt 2>&1`
Expected: all tests PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/voxup/tests/distribution_parity.rs
git commit -m "test(dist): SSOT-declared binaries map to real crate dirs"
```

---

## Task 8: Wire the gate into CI `[SEQUENTIAL]`

**Files:**
- Create: `.github/workflows/distribution-parity.yml`

- [ ] **Step 1: Create the workflow**

Create `.github/workflows/distribution-parity.yml`:

```yaml
# Distribution SSOT parity gate (Track 0). Hosted-only so it never depends on
# the self-hosted fleet. Make this a required check to enforce.
name: distribution-parity

on:
  pull_request:
    paths:
      - "contracts/distribution/**"
      - "contracts/toolchain/**"
      - "crates/_public.toml"
      - "crates/voxup/**"
      - ".github/workflows/distribution-parity.yml"
  push:
    branches: [main]

permissions:
  contents: read

concurrency:
  group: distribution-parity-${{ github.ref }}
  cancel-in-progress: true

jobs:
  parity:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@stable
      - name: Run distribution SSOT parity tests
        run: cargo test -p voxup --test distribution_parity --locked
```

- [ ] **Step 2: Lint the workflow locally (if `actionlint` available)**

Run: `actionlint .github/workflows/distribution-parity.yml` if installed; otherwise skip (CI `workflow-lint.yml` will catch issues).

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/distribution-parity.yml
git commit -m "ci(dist): required parity gate for distribution SSOT"
```

---

## Task 9: Document the SSOT `[PARALLEL-SAFE]`

**Files:**
- Create: `docs/src/architecture/distribution-ssot.md`

- [ ] **Step 1: Write the doc**

Create `docs/src/architecture/distribution-ssot.md` with frontmatter (required under `docs/src/` per AGENTS.md):

```markdown
---
title: "Distribution SSOT"
description: "The single source of truth for install tiers, dependency closures, the crates.io publish set, and released binaries."
category: "Architecture SSOTs"
status: "current"
---

# Distribution SSOT

`contracts/distribution/profiles.v1.yaml` is the single source of truth for how
Vox is installed, released, and (eventually) published.

## What it governs

- **Tiers** — `minimal` / `default` / `full`, each with its binary set, build
  deps, and runtime-optional deps. `voxup install <tier>` and `vox doctor` read this.
- **Publish set** — the crates.io publish list (leaf-first), reconciled against
  `crates/_public.toml`. `publish.enabled: false` keeps the public flip deferred.
- **Binaries** — what release + nightly build (`vox`, `vox-ml-cli`, `voxup`).
- **agy containment** — `agy` is runtime-optional in the `full` tier only;
  `vox-orchestrator-mcp` is non-publishable. Enforced by the parity test.

## Enforcement

`crates/voxup/tests/distribution_parity.rs` (CI: `distribution-parity.yml`)
fails when the manifest drifts from: the toolchain contract, `_public.toml`,
the agy-containment rule, or the on-disk crate dirs.

## Consumers (Tracks A–D)

- Track A — `voxup install <tier>` + `vox doctor` per-tier dep enable.
- Track B — release + nightly read `binaries`.
- Track C — publish automation reads `publish`.
- Track D — supply-chain trust.
```

- [ ] **Step 2: Commit**

```bash
git add docs/src/architecture/distribution-ssot.md
git commit -m "docs(dist): document the distribution SSOT and its parity gate"
```

---

## Self-Review notes

- **Spec coverage:** Implements the spec's "Architecture — the spine" (SSOT manifest) and the agy-containment hard rules (Task 4 `agy_only_in_full_tier_runtime_optional`, Task 1 `non_publishable`). Rust-version-skew and the publish-set reconciliation gates are covered (Tasks 5, 6). Tracks A–D are deliberately out of this plan (separate plans).
- **No new scripts:** all automation is Rust tests + one YAML workflow (AGENTS.md compliant).
- **Type consistency:** `Profiles`/`Tier`/`Publish` structs in Task 3 are reused verbatim by the tests in Tasks 4–7 via `voxup::profiles::*`.
- **Known soft spots for the critique session:** (a) the `contains`-based toolchain check in Task 5 is loose; tighten after seeing the contract's real key. (b) `_public.toml` relative path in Task 6 should be confirmed by first run. (c) whether to host the reader in `voxup` vs a dedicated crate — revisit if Track C needs the reader without pulling `voxup`.
```
