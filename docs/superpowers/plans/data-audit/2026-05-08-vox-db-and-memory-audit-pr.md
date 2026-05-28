# Vox-DB & Memory Management Audit — Single-PR Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land a single PR that closes the policy↔enforcement gap for Turso usage, mechanically prevents future drift, makes `vox-db-types` actually useful as an L0 types crate, standardizes serde + From/TryFrom on row types, introduces typed string-ID newtypes, and updates documentation.

**Architecture:** The work is sequenced so every phase is independently testable and committable. Earlier phases tighten guards (defense-in-depth) before later phases refactor types — that way the type refactors run *through* the new guards. We follow TDD: each new check or guard ships with a failing test first, then the code, then the policy/data update that makes the test pass on the real workspace.

**Tech Stack:** Rust 2024 (workspace), Turso (libSQL), `cargo` workspace + `vox-arch-check`, `vox ci` guard suite (`turso-import-guard`, `data-storage-guard`, `query-all-guard`, `sql-surface-guard`), serde, thiserror, anyhow, regex, glob, serde_yaml.

**Scope-check note:** The user explicitly requested this all in one PR ("low yield is fine"). The plan respects that, but each commit is reviewable on its own. If the PR grows beyond ~3000 changed lines, the executor MAY split off Phase 4 (row-type relocation) and Phase 6/7 (typed bridges) as a follow-up PR — Phases 0-3, 5, 8-10 are the consistency-and-guard story and stand alone.

---

## Pre-flight Required Reading (assume zero context)

Before starting, the executor MUST read:

1. [`AGENTS.md`](../../../../AGENTS.md) — cross-tool policy.
2. [`CLAUDE.md`](../../../../CLAUDE.md) — Claude-specific overlay; especially the "consult `where-things-live.md` before adding code" and "VoxScript-first glue code" rules.
3. [`docs/src/architecture/where-things-live.md`](../../../src/architecture/where-things-live.md) — flat lookup table.
4. [`docs/src/architecture/layers.toml`](../../../src/architecture/layers.toml) — layer rules + `[[known_inversions]]` pattern (re-used in this plan).
5. [`docs/src/adr/004-codex-arca-turso-ssot.md`](../../../src/adr/004-codex-arca-turso-ssot.md) — Turso-as-SSOT decision.
6. [`contracts/db/data-storage-policy.v1.yaml`](../../../../contracts/db/data-storage-policy.v1.yaml) — declarative policy; Tasks below treat this as authoritative when prose disagrees.
7. [`docs/agents/turso-import-allowlist.txt`](../../../agents/turso-import-allowlist.txt) — current transitional allowlist.
8. [`docs/agents/database-nomenclature.md`](../../../agents/database-nomenclature.md) — VoxDb vs Codex vs Arca naming.

If any of these are missing or moved, STOP and report — that itself is drift.

---

## Workflow Discipline

- **TDD throughout.** Write the failing test first, run it to confirm failure, then write code, then re-run.
- **Commit after every passing task** (not after every step). Commit messages follow conventional-commit style: `fix(vox-secrets):`, `feat(vox-db-types):`, `chore(docs):`, etc.
- **Never edit auto-generated docs by hand** (`SUMMARY.md`, `architecture-index.md`, `feed.xml`, `*.generated.md`, `.cursorignore`). Re-run the generator instead. See user memory `feedback_auto_generated_docs.md`.
- **Use `.vox` for any new automation script** per CLAUDE.md / AGENTS.md "VoxScript-First Glue Code". Do not generate `.ps1`, `.sh`, or `.py`.
- **After every multi-file edit**, run `cargo check --workspace` before moving on. If it fails, fix before next step.
- **Read before assuming.** If a file's contents differ from this plan (e.g. line numbers shifted), re-grep for the anchor string before editing.

---

# PHASE 0 — Baseline & Investigation

Establish ground truth before changing anything. The audit that produced this plan was based on subagent reports; verify the load-bearing claims firsthand.

### Task 0.1: Verify current guard pass/fail state

**Files:**
- Read-only: `crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs`
- Read-only: `crates/vox-cli/tests/turso_import_guard_integration.rs`
- Read-only: `docs/agents/turso-import-allowlist.txt`

- [ ] **Step 1: Build the `vox` binary so the integration test can run**

```bash
cargo build -p vox-cli --bin vox
```

Expected: success. If this fails the whole plan stops here — fix the build first.

- [ ] **Step 2: Run the existing turso-import-guard integration test**

```bash
cargo test -p vox-cli --test turso_import_guard_integration -- --nocapture
```

Expected: PASS (test asserts `vox ci turso-import-guard --all` exits 0).

- [ ] **Step 3: Run the guard manually and capture which crates currently pass**

```bash
cargo run -p vox-cli --bin vox -- ci turso-import-guard --all 2>&1 | tee /tmp/turso-guard-baseline.txt
```

Expected: exit 0 and the line `turso-import-guard OK`.

- [ ] **Step 4: Confirm `vox-secrets` actually contains a `turso::` symbol**

```bash
rg --line-number "\btur" crates/vox-secrets/src
```

Expected: hits in `crates/vox-secrets/src/backend/vox_vault.rs`. If zero hits, the audit's premise was wrong — STOP and re-evaluate.

- [ ] **Step 5: Determine why the guard passes despite vox-secrets containing `turso::`**

The guard's `scan_targets` may exclude `vox-secrets` via a non-obvious mechanism (e.g. directory scope, file extension, comment-stripping, or an inherited prefix match). Open `crates/vox-cli/src/commands/ci/run_body_helpers/matrix.rs` (the source of `visit_rs_files` referenced from `guards.rs:7`) and read it.

```bash
sed -n '1,200p' crates/vox-cli/src/commands/ci/run_body_helpers/matrix.rs
```

Record findings in `docs/superpowers/plans/data-audit/2026-05-08-vox-db-audit-baseline.md` (a sibling notes file, ≤30 lines). Capture: the actual scan scope, whether `vox-secrets` is included, and whether the regex matches the contents of `vox_vault.rs`.

- [ ] **Step 6: Re-run with verbose tracing to confirm**

If still ambiguous, add `--verbose` if the subcommand supports it, OR temporarily prepend a `dbg!(rel_norm)` inside `run_turso_import_guard` and re-run, then revert. Do NOT commit the dbg!.

- [ ] **Step 7: Commit the baseline notes file**

```bash
git add docs/superpowers/plans/data-audit/2026-05-08-vox-db-audit-baseline.md
git commit -m "docs(plan): baseline notes for vox-db audit PR"
```

### Task 0.2: Snapshot the workspace's CREATE TABLE inventory

We need a known-good list of every `CREATE TABLE` in the workspace before adding the schema-coverage check (Phase 2). This snapshot becomes the test fixture.

- [ ] **Step 1: Generate the inventory**

```bash
mkdir -p crates/vox-cli/tests/fixtures/db-schema-coverage
rg --no-heading --line-number 'CREATE\s+TABLE\s+(IF\s+NOT\s+EXISTS\s+)?[a-zA-Z_][a-zA-Z0-9_]*' \
   --glob '!**/target/**' \
   --glob '!**/.git/**' \
   crates/ contracts/ \
  > crates/vox-cli/tests/fixtures/db-schema-coverage/raw-inventory.txt
```

- [ ] **Step 2: Extract just the table names (sorted, unique)**

Use a `.vox` script per VoxScript-first rule. Create `scripts/extract_table_names.vox`:

```vox
// Extracts unique table names from a CREATE TABLE inventory file.
// Reads stdin or first arg; writes sorted unique names to stdout.
import std/io
import std/regex
import std/string

let path = std.env.args().get(1).expect("usage: extract_table_names.vox <inventory-path>")
let raw = std.io.read_file(path)
let re = std.regex.compile(r"CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?([a-zA-Z_][a-zA-Z0-9_]*)")
let names = []
for line in std.string.split(raw, "\n") {
    if let Some(m) = re.find(line) {
        names.push(m.group(1))
    }
}
let unique = std.string.sort(std.string.unique(names))
for n in unique {
    std.io.println(n)
}
```

If `std/regex` or other stdlib pieces don't yet exist in Vox, fall back to `cargo run -p vox-cli -- run scripts/extract_table_names.vox`. If even that fails because Vox lacks the stdlib pieces, embed the extraction inline in the test (Step 4 below) using `regex::Regex` instead — DO NOT write a `.ps1`, `.sh`, or `.py` script.

- [ ] **Step 3: Run the extractor and review the output**

```bash
cargo run -p vox-cli --bin vox -- run scripts/extract_table_names.vox \
  crates/vox-cli/tests/fixtures/db-schema-coverage/raw-inventory.txt \
  > crates/vox-cli/tests/fixtures/db-schema-coverage/known-tables.txt
```

Sanity-check: should include obvious names like `memories`, `embeddings`, `clavis_account_secrets`, `objects`, etc. If empty or near-empty, the extractor is broken — debug before proceeding.

- [ ] **Step 4: Commit the fixture and the extractor**

```bash
git add scripts/extract_table_names.vox crates/vox-cli/tests/fixtures/db-schema-coverage/
git commit -m "chore(db-audit): snapshot known CREATE TABLE inventory for coverage check"
```

---

# PHASE 1 — Close the vox-secrets policy↔enforcement gap

The `data-storage-policy.v1.yaml` lists `vox-secrets` in `tiers.a_relational.allow_direct_access`, but `docs/agents/turso-import-allowlist.txt` does NOT. Either the guard already exempts `vox-secrets` via a different mechanism (which Task 0.1 will reveal), or there's a true gap. This phase aligns the two surfaces.

### Task 1.1: Add vox-secrets to the turso-import-guard allowlist

**Files:**
- Modify: `docs/agents/turso-import-allowlist.txt`

- [ ] **Step 1: Read the current file**

Already loaded earlier; confirm exact contents:

```bash
cat docs/agents/turso-import-allowlist.txt
```

- [ ] **Step 2: Append `vox-secrets` with a justification comment**

Use Edit to add (preserve trailing newline). Replace the final non-empty line:

```text
crates/vox-populi/src/transport/store/
```

with:

```text
crates/vox-populi/src/transport/store/
# vox-secrets: tier-A relational owner per contracts/db/data-storage-policy.v1.yaml.
# Clavis vault uses a separate libSQL DB file (.vox/clavis_vault.db) for blast-radius
# isolation from user-data Codex; intentional and documented in ADR-004.
crates/vox-secrets/
```

- [ ] **Step 3: Re-run the guard and the integration test**

```bash
cargo run -p vox-cli --bin vox -- ci turso-import-guard --all
cargo test -p vox-cli --test turso_import_guard_integration
```

Expected: both still pass.

- [ ] **Step 4: Commit**

```bash
git add docs/agents/turso-import-allowlist.txt
git commit -m "fix(turso-guard): align allowlist with data-storage-policy (vox-secrets owner)"
```

### Task 1.2: Add a CI test that turso-import-allowlist matches the policy YAML

This is the actual mechanical drift-detector: any future divergence between the policy YAML's `allow_direct_access` and the allowlist file fails CI.

**Files:**
- Create: `crates/vox-cli/src/commands/ci/policy_allowlist_parity.rs`
- Modify: `crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs` (call new check from `run_turso_import_guard` epilogue, OR add separate subcommand)
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` (add subcommand variant if applicable)
- Modify: `crates/vox-cli/src/commands/ci/run_body.rs` (dispatch new subcommand)
- Create: `crates/vox-cli/tests/policy_allowlist_parity_integration.rs`

- [ ] **Step 1: Write the failing integration test**

Create `crates/vox-cli/tests/policy_allowlist_parity_integration.rs`:

```rust
//! Integration: every crate in `tiers.a_relational.allow_direct_access` of
//! `contracts/db/data-storage-policy.v1.yaml` must appear (as a `crates/<name>/`
//! prefix) in `docs/agents/turso-import-allowlist.txt`, OR be one of the built-in
//! prefixes hard-coded in `run_body_helpers/guards.rs::load_turso_import_allowlist`.

use std::path::Path;
use std::process::Command;

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("workspace root")
}

#[test]
fn policy_allowlist_parity_passes_on_main() {
    let bin = env!("CARGO_BIN_EXE_vox");
    let out = Command::new(bin)
        .current_dir(workspace_root())
        .args(["ci", "policy-allowlist-parity"])
        .output()
        .expect("spawn vox ci policy-allowlist-parity");
    assert!(
        out.status.success(),
        "policy-allowlist-parity should exit 0; stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
```

- [ ] **Step 2: Run it to verify it fails**

```bash
cargo test -p vox-cli --test policy_allowlist_parity_integration
```

Expected: FAIL — subcommand does not exist yet.

- [ ] **Step 3: Add the subcommand variant**

Open `crates/vox-cli/src/commands/ci/cmd_enums.rs`, find the `enum CiSubcommand` (or similar) and add a variant. Read the file to find the exact pattern in use:

```bash
rg -n 'TursoImportGuard|turso-import-guard' crates/vox-cli/src/commands/ci/cmd_enums.rs
```

Add a sibling variant `PolicyAllowlistParity` (no args) following the same syntax as `TursoImportGuard`. If the enum uses `clap` derive macros, mirror the attributes. If it uses a manual match, mirror that.

- [ ] **Step 4: Implement the parity check**

Create `crates/vox-cli/src/commands/ci/policy_allowlist_parity.rs`:

```rust
//! `vox ci policy-allowlist-parity` — verifies that every crate in the
//! `tiers.a_relational.allow_direct_access` list of
//! `contracts/db/data-storage-policy.v1.yaml` is reachable from
//! `docs/agents/turso-import-allowlist.txt` (plus the built-in guard prefixes).

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const BUILTIN_PREFIXES: &[&str] = &["vox-db", "vox-package", "vox-compiler"];

#[derive(Debug, Deserialize)]
struct Policy {
    tiers: Tiers,
}

#[derive(Debug, Deserialize)]
struct Tiers {
    a_relational: TierA,
}

#[derive(Debug, Deserialize)]
struct TierA {
    #[serde(default)]
    allow_direct_access: Vec<String>,
    #[serde(default)]
    temporary_exceptions: Vec<String>,
}

pub fn run(root: &Path) -> Result<()> {
    let policy_path = root.join("contracts/db/data-storage-policy.v1.yaml");
    let yaml = fs::read_to_string(&policy_path)
        .with_context(|| format!("read {}", policy_path.display()))?;
    let policy: Policy = serde_yaml::from_str(&yaml).context("parse data-storage policy")?;

    let allowlist_path = root.join("docs/agents/turso-import-allowlist.txt");
    let allowlist_text = fs::read_to_string(&allowlist_path)
        .with_context(|| format!("read {}", allowlist_path.display()))?;
    let allowlist_crates: BTreeSet<String> = allowlist_text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.strip_prefix("crates/").map(|s| s.trim_end_matches('/').to_string()))
        .map(|s| s.split('/').next().unwrap_or(&s).to_string())
        .collect();

    let mut policy_crates: BTreeSet<String> = policy.tiers.a_relational.allow_direct_access
        .iter()
        .cloned()
        .collect();
    policy_crates.extend(policy.tiers.a_relational.temporary_exceptions.iter().cloned());

    let mut missing: Vec<String> = Vec::new();
    for c in &policy_crates {
        if BUILTIN_PREFIXES.contains(&c.as_str()) {
            continue;
        }
        if !allowlist_crates.contains(c) {
            missing.push(c.clone());
        }
    }

    if !missing.is_empty() {
        return Err(anyhow!(
            "policy-allowlist-parity: crates listed in data-storage-policy.v1.yaml \
             tiers.a_relational.allow_direct_access (or temporary_exceptions) but missing \
             from docs/agents/turso-import-allowlist.txt: {}. Add a `crates/<name>/` line \
             with a justification comment, OR remove from the policy if the crate no longer \
             needs direct turso access.",
            missing.join(", ")
        ));
    }

    println!("policy-allowlist-parity OK ({} policy crates checked)", policy_crates.len());
    Ok(())
}
```

- [ ] **Step 5: Wire dispatch**

Open `crates/vox-cli/src/commands/ci/run_body.rs`. Find where `TursoImportGuard` dispatches:

```bash
rg -n 'TursoImportGuard|turso-import-guard|run_turso_import_guard' crates/vox-cli/src/commands/ci/run_body.rs
```

Add a sibling arm dispatching `PolicyAllowlistParity` to the new module's `run` function. The dispatch likely looks like (adapt to actual code):

```rust
CiSubcommand::TursoImportGuard { all } => {
    super::run_body_helpers::guards::run_turso_import_guard(&root, all)?;
}
CiSubcommand::PolicyAllowlistParity => {
    super::policy_allowlist_parity::run(&root)?;
}
```

- [ ] **Step 6: Add the module to mod.rs**

Open `crates/vox-cli/src/commands/ci/mod.rs` (or wherever the ci module declarations live — `rg -n 'pub mod' crates/vox-cli/src/commands/ci/mod.rs` to find the pattern) and add:

```rust
pub mod policy_allowlist_parity;
```

- [ ] **Step 7: Register the command in command-registry contracts (if applicable)**

```bash
rg -n 'turso-import-guard' contracts/cli/command-registry.yaml
```

If the file lists subcommands, add `policy-allowlist-parity` mirroring the existing `turso-import-guard` entry (same fields, no flags). If the registry has a `description`, use: `Verify allow_direct_access in data-storage-policy.v1.yaml matches docs/agents/turso-import-allowlist.txt.`

- [ ] **Step 8: Run the integration test — should now pass**

```bash
cargo test -p vox-cli --test policy_allowlist_parity_integration
```

Expected: PASS.

- [ ] **Step 9: Add a unit test for the failure path**

Append to the bottom of `crates/vox-cli/src/commands/ci/policy_allowlist_parity.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(p: &std::path::Path, content: &str) {
        if let Some(parent) = p.parent() { fs::create_dir_all(parent).unwrap(); }
        fs::write(p, content).unwrap();
    }

    #[test]
    fn fails_when_policy_lists_crate_not_in_allowlist() {
        let td = tempdir().unwrap();
        let root = td.path();
        write(&root.join("contracts/db/data-storage-policy.v1.yaml"),
            "tiers:\n  a_relational:\n    allow_direct_access: [vox-db, vox-mystery]\n");
        write(&root.join("docs/agents/turso-import-allowlist.txt"),
            "crates/vox-secrets/\n");
        let err = run(root).unwrap_err().to_string();
        assert!(err.contains("vox-mystery"), "error must name the missing crate; got: {err}");
    }

    #[test]
    fn passes_when_builtin_prefixes_cover_policy() {
        let td = tempdir().unwrap();
        let root = td.path();
        write(&root.join("contracts/db/data-storage-policy.v1.yaml"),
            "tiers:\n  a_relational:\n    allow_direct_access: [vox-db, vox-package, vox-compiler]\n");
        write(&root.join("docs/agents/turso-import-allowlist.txt"), "");
        run(root).expect("builtin prefixes should satisfy parity");
    }
}
```

If `tempfile` is not already a `dev-dependency` of `vox-cli`, add `tempfile = { workspace = true }` under `[dev-dependencies]` in `crates/vox-cli/Cargo.toml`. Verify it's in `Cargo.toml` `[workspace.dependencies]` first (`rg -n '^tempfile' Cargo.toml`).

- [ ] **Step 10: Run all unit tests for the new module**

```bash
cargo test -p vox-cli policy_allowlist_parity
```

Expected: PASS for both unit tests + the integration test.

- [ ] **Step 11: Commit**

```bash
git add crates/vox-cli/src/commands/ci/policy_allowlist_parity.rs \
        crates/vox-cli/src/commands/ci/cmd_enums.rs \
        crates/vox-cli/src/commands/ci/run_body.rs \
        crates/vox-cli/src/commands/ci/mod.rs \
        crates/vox-cli/tests/policy_allowlist_parity_integration.rs \
        contracts/cli/command-registry.yaml \
        crates/vox-cli/Cargo.toml
git commit -m "feat(ci): add policy-allowlist-parity check (data-storage-policy ↔ turso-import-allowlist)"
```

### Task 1.3: Wire the parity check into the umbrella `vox ci` runs

If there's a `vox ci all` or `vox ci guards` umbrella that runs every guard, add this one. Otherwise skip.

- [ ] **Step 1: Find the umbrella**

```bash
rg -n 'TursoImportGuard|run_turso_import_guard' crates/vox-cli/src/commands/ci/run_body.rs crates/vox-cli/src/commands/ci/run_body_helpers/
```

If `run_repo_guards` (in `guards.rs`) chains every guard, add a call to `super::policy_allowlist_parity::run(root)?` there. If guards run via separate `vox ci <name>` invocations only, skip this task.

- [ ] **Step 2: If chained, run all guards**

```bash
cargo run -p vox-cli --bin vox -- ci turso-import-guard --all
```

Expected: the new parity output appears in the chained run.

- [ ] **Step 3: Commit if changed**

```bash
git add crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs
git commit -m "chore(ci): chain policy-allowlist-parity into umbrella guard run"
```

---

# PHASE 2 — Schema-coverage CI check

Catches: a new `CREATE TABLE` introduced anywhere in the workspace that's not registered in `vox-db`'s `SCHEMA_FRAGMENTS` manifest (or in an explicitly-allowlisted owner crate's schema file). Would have flagged the `clavis_account_secrets` tables as out-of-band if they hadn't been ADR-blessed.

### Task 2.1: Failing test for db-schema-coverage

**Files:**
- Create: `crates/vox-cli/tests/db_schema_coverage_integration.rs`
- Create: `crates/vox-cli/src/commands/ci/db_schema_coverage.rs` (next task)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli/tests/db_schema_coverage_integration.rs`:

```rust
//! Integration: every CREATE TABLE in the workspace either
//!  (a) is owned by `vox-db` and reachable from `SCHEMA_FRAGMENTS`, OR
//!  (b) lives under a crate listed as an owner in
//!      `contracts/db/data-storage-policy.v1.yaml` tiers.a_relational.owners.
//! Anything else is a coverage gap and fails CI.

use std::path::Path;
use std::process::Command;

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().expect("crates/")
        .parent().expect("workspace root")
}

#[test]
fn db_schema_coverage_passes() {
    let bin = env!("CARGO_BIN_EXE_vox");
    let out = Command::new(bin)
        .current_dir(workspace_root())
        .args(["ci", "db-schema-coverage"])
        .output()
        .expect("spawn vox ci db-schema-coverage");
    assert!(
        out.status.success(),
        "db-schema-coverage should exit 0;\nstdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
```

- [ ] **Step 2: Confirm it fails**

```bash
cargo test -p vox-cli --test db_schema_coverage_integration
```

Expected: FAIL (subcommand missing).

### Task 2.2: Implement the schema-coverage check

**Files:**
- Create: `crates/vox-cli/src/commands/ci/db_schema_coverage.rs`
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` (subcommand variant)
- Modify: `crates/vox-cli/src/commands/ci/run_body.rs` (dispatch)
- Modify: `crates/vox-cli/src/commands/ci/mod.rs` (module decl)
- Modify: `contracts/cli/command-registry.yaml` (registry entry)

- [ ] **Step 1: Add the subcommand variant**

Mirror the pattern used for `PolicyAllowlistParity` from Phase 1 Task 1.2 Step 3. Add a `DbSchemaCoverage` variant.

- [ ] **Step 2: Add the module declaration**

In `crates/vox-cli/src/commands/ci/mod.rs`:

```rust
pub mod db_schema_coverage;
```

- [ ] **Step 3: Implement the check**

Create `crates/vox-cli/src/commands/ci/db_schema_coverage.rs`:

```rust
//! `vox ci db-schema-coverage` — verifies every CREATE TABLE in the workspace
//! is owned by a crate listed in `tiers.a_relational.owners` of
//! `contracts/db/data-storage-policy.v1.yaml`.
//!
//! This is the mechanical version of "no parallel persistence layers" — if a
//! crate adds a new table, its crate name must appear in `owners` (or
//! `temporary_exceptions`), forcing the policy update to land in the same PR.

use anyhow::{Context, Result, anyhow};
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct Policy {
    tiers: Tiers,
}

#[derive(Debug, Deserialize)]
struct Tiers {
    a_relational: TierA,
}

#[derive(Debug, Deserialize)]
struct TierA {
    #[serde(default)]
    owners: Vec<String>,
    #[serde(default)]
    temporary_exceptions: Vec<String>,
}

#[derive(Debug)]
struct Hit {
    crate_name: String,
    file: PathBuf,
    line: usize,
    table: String,
}

pub fn run(root: &Path) -> Result<()> {
    let policy_path = root.join("contracts/db/data-storage-policy.v1.yaml");
    let yaml = fs::read_to_string(&policy_path)
        .with_context(|| format!("read {}", policy_path.display()))?;
    let policy: Policy = serde_yaml::from_str(&yaml).context("parse policy yaml")?;

    let mut allowed: BTreeSet<String> =
        policy.tiers.a_relational.owners.iter().cloned().collect();
    allowed.extend(policy.tiers.a_relational.temporary_exceptions.iter().cloned());

    let create_re = Regex::new(
        r"(?im)^\s*CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?([a-zA-Z_][a-zA-Z0-9_]*)"
    ).expect("create_table regex");

    let crates_dir = root.join("crates");
    let mut hits: Vec<Hit> = Vec::new();
    walk(&crates_dir, &mut |path| {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !(name.ends_with(".rs") || name.ends_with(".sql")) {
            return Ok(());
        }
        // skip generated/target
        let path_s = path.to_string_lossy();
        if path_s.contains("/target/") || path_s.contains("\\target\\") {
            return Ok(());
        }
        let body = match fs::read_to_string(path) {
            Ok(b) => b, Err(_) => return Ok(()),
        };
        if !body.contains("CREATE") { return Ok(()); }
        let crate_name = crate_of(path, &crates_dir).unwrap_or_default();
        for (line_idx, line) in body.lines().enumerate() {
            if let Some(c) = create_re.captures(line) {
                hits.push(Hit {
                    crate_name: crate_name.clone(),
                    file: path.to_path_buf(),
                    line: line_idx + 1,
                    table: c.get(1).unwrap().as_str().to_string(),
                });
            }
        }
        Ok(())
    })?;

    let mut violations: Vec<String> = Vec::new();
    for h in &hits {
        if allowed.contains(&h.crate_name) { continue; }
        violations.push(format!(
            "  {}:{}  table `{}` in crate `{}` (not in tiers.a_relational.owners)",
            h.file.strip_prefix(root).unwrap_or(&h.file).display(),
            h.line, h.table, h.crate_name,
        ));
    }

    if !violations.is_empty() {
        return Err(anyhow!(
            "db-schema-coverage: {} CREATE TABLE statement(s) in non-owner crates:\n{}\n\nFix: add the crate to `tiers.a_relational.owners` in contracts/db/data-storage-policy.v1.yaml (and to docs/agents/turso-import-allowlist.txt), or move the schema into vox-db's SCHEMA_FRAGMENTS.",
            violations.len(),
            violations.join("\n"),
        ));
    }

    println!("db-schema-coverage OK ({} CREATE TABLE statements, all in owner crates)", hits.len());
    Ok(())
}

fn walk(dir: &Path, f: &mut dyn FnMut(&Path) -> Result<()>) -> Result<()> {
    if !dir.is_dir() { return Ok(()); }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(name, "target" | ".git" | "node_modules" | "dist") { continue; }
            walk(&path, f)?;
        } else {
            f(&path)?;
        }
    }
    Ok(())
}

fn crate_of(file: &Path, crates_dir: &Path) -> Option<String> {
    let rel = file.strip_prefix(crates_dir).ok()?;
    rel.components().next()
        .and_then(|c| c.as_os_str().to_str())
        .map(String::from)
}
```

- [ ] **Step 4: Wire dispatch**

Add to `run_body.rs` mirroring the parity check from Phase 1:

```rust
CiSubcommand::DbSchemaCoverage => {
    super::db_schema_coverage::run(&root)?;
}
```

- [ ] **Step 5: Add to command registry**

In `contracts/cli/command-registry.yaml`, mirror the `turso-import-guard` entry. Description: `Verify every CREATE TABLE in the workspace is owned by a crate in tiers.a_relational.owners.`

- [ ] **Step 6: Run the integration test on the current workspace**

```bash
cargo test -p vox-cli --test db_schema_coverage_integration -- --nocapture
```

If it FAILS, the failure list IS the diagnostic — note which crates contain CREATE TABLE statements. Compare against the policy's `owners`. Likely findings: tests/fixtures, in-line schema strings in places that are legitimate but need the policy to acknowledge them.

- [ ] **Step 7: Reconcile findings**

For each failing crate:
- If it's a test/fixture (`crates/*/tests/`), update the walker to skip `tests/` directories and document why (test fixtures are not production schema).
- If it's a real crate not in policy, **STOP** — escalate to user. Do not just add it to the policy without review.
- If it's `vox-db`, expected — already an owner.

The walker exclusion for tests:

```rust
// at the top of walk(): also skip test directories
if matches!(name, "target" | ".git" | "node_modules" | "dist" | "tests" | "fixtures") { continue; }
```

Re-run Step 6 until it passes (or escalate).

- [ ] **Step 8: Add unit tests for the policy parser**

At the bottom of `db_schema_coverage.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn flags_table_in_unowned_crate() {
        let td = tempdir().unwrap();
        let root = td.path();
        fs::create_dir_all(root.join("contracts/db")).unwrap();
        fs::write(root.join("contracts/db/data-storage-policy.v1.yaml"),
            "tiers:\n  a_relational:\n    owners: [vox-db]\n").unwrap();
        fs::create_dir_all(root.join("crates/vox-rogue/src")).unwrap();
        fs::write(root.join("crates/vox-rogue/src/lib.rs"),
            "fn x() { let _ = \"CREATE TABLE rogue_table (id INT)\"; }").unwrap();
        let err = run(root).unwrap_err().to_string();
        assert!(err.contains("rogue_table"));
        assert!(err.contains("vox-rogue"));
    }

    #[test]
    fn passes_when_table_in_owner_crate() {
        let td = tempdir().unwrap();
        let root = td.path();
        fs::create_dir_all(root.join("contracts/db")).unwrap();
        fs::write(root.join("contracts/db/data-storage-policy.v1.yaml"),
            "tiers:\n  a_relational:\n    owners: [vox-db]\n").unwrap();
        fs::create_dir_all(root.join("crates/vox-db/src")).unwrap();
        fs::write(root.join("crates/vox-db/src/x.rs"),
            "const S: &str = \"CREATE TABLE memories (id INT)\";").unwrap();
        run(root).expect("table in owner crate should pass");
    }
}
```

- [ ] **Step 9: Run unit tests**

```bash
cargo test -p vox-cli db_schema_coverage
```

Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/vox-cli/src/commands/ci/db_schema_coverage.rs \
        crates/vox-cli/src/commands/ci/cmd_enums.rs \
        crates/vox-cli/src/commands/ci/run_body.rs \
        crates/vox-cli/src/commands/ci/mod.rs \
        crates/vox-cli/tests/db_schema_coverage_integration.rs \
        contracts/cli/command-registry.yaml
git commit -m "feat(ci): db-schema-coverage check (every CREATE TABLE must live in an owner crate)"
```

### Task 2.3: Add coverage check to umbrella

- [ ] **Step 1: Chain the check into `run_repo_guards` if appropriate**

Same pattern as Task 1.3 — mirror the placement.

- [ ] **Step 2: Verify**

```bash
cargo run -p vox-cli --bin vox -- ci db-schema-coverage
```

Expected: `db-schema-coverage OK (...)`.

- [ ] **Step 3: Commit if changed**

```bash
git commit -am "chore(ci): chain db-schema-coverage into umbrella guard run"
```

---

# PHASE 3 — Make policy YAML the single source of truth

Today the built-in prefixes in `guards.rs::load_turso_import_allowlist` (`vox-db`, `vox-package`, `vox-compiler`) are hard-coded in Rust *and* duplicated in the YAML. This phase removes the duplication: the Rust code reads from the YAML.

### Task 3.1: Refactor `load_turso_import_allowlist` to derive built-ins from policy

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs`

- [ ] **Step 1: Read the current implementation**

```bash
sed -n '141,200p' crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs
```

- [ ] **Step 2: Add a failing test for "policy crate auto-included"**

In the same file's `#[cfg(test)] mod tests` (create one if missing), add:

```rust
#[test]
fn allowlist_includes_policy_owners_without_txt_entry() {
    // tiers.a_relational.owners includes vox-secrets per data-storage-policy.v1.yaml.
    // load_turso_import_allowlist must include `crates/vox-secrets/` even if the .txt
    // file does not list it.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap();
    let allow = load_turso_import_allowlist(root).unwrap();
    assert!(allow.iter().any(|p| p == "crates/vox-secrets/"),
        "expected crates/vox-secrets/ in allowlist; got {:?}", allow);
}
```

- [ ] **Step 3: Run — should fail**

```bash
cargo test -p vox-cli allowlist_includes_policy_owners_without_txt_entry
```

Expected: FAIL.

- [ ] **Step 4: Modify `load_turso_import_allowlist` to merge in policy owners**

Find the function (around line 141). Replace its body so it ALSO reads `contracts/db/data-storage-policy.v1.yaml` and adds `crates/<owner>/` for each entry in `tiers.a_relational.owners` and `tiers.a_relational.allow_direct_access`. Sketch:

```rust
fn load_turso_import_allowlist(root: &Path) -> Result<Vec<String>> {
    let mut out = vec![
        "crates/vox-db/".to_string(),
        "crates/vox-package/".to_string(),
        "crates/vox-compiler/".to_string(),
    ];

    // Merge owners from the data-storage policy YAML.
    let policy_path = root.join("contracts/db/data-storage-policy.v1.yaml");
    if policy_path.is_file() {
        if let Ok(yaml) = std::fs::read_to_string(&policy_path) {
            if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(&yaml) {
                let lists = [
                    val.get("tiers").and_then(|t| t.get("a_relational")).and_then(|t| t.get("owners")),
                    val.get("tiers").and_then(|t| t.get("a_relational")).and_then(|t| t.get("allow_direct_access")),
                    val.get("tiers").and_then(|t| t.get("a_relational")).and_then(|t| t.get("temporary_exceptions")),
                ];
                for list in lists.into_iter().flatten() {
                    if let Some(seq) = list.as_sequence() {
                        for item in seq {
                            if let Some(name) = item.as_str() {
                                out.push(format!("crates/{name}/"));
                            }
                        }
                    }
                }
            }
        }
    }

    // Existing transitional allowlist file
    let p = root.join("docs/agents/turso-import-allowlist.txt");
    if p.is_file() {
        let text = read_utf8_path_capped(&p).with_context(|| format!("read {}", p.display()))?;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let norm = line.replace('\\', "/");
            let norm = if norm.ends_with('/') { norm } else { format!("{norm}/") };
            out.push(norm);
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}
```

If `serde_yaml` is not in `vox-cli/Cargo.toml`'s dependencies, add it (`serde_yaml = { workspace = true }`). Verify `serde_yaml` is in workspace dependencies first.

- [ ] **Step 5: Run the new test plus existing tests**

```bash
cargo test -p vox-cli -- --nocapture
```

Expected: PASS for the new test plus all existing turso-import-guard tests.

- [ ] **Step 6: Now redundant: prune the duplicate `vox-secrets` line from the txt file**

Now that the policy YAML alone is sufficient, the txt entry is redundant. Edit `docs/agents/turso-import-allowlist.txt` and remove the `crates/vox-secrets/` line and its comment block (added in Phase 1.1). Same for any other `crates/<name>/` line that duplicates a policy owner.

After edit, file should contain only the *transitional* exceptions that aren't sanctioned policy owners.

- [ ] **Step 7: Run guards again to confirm nothing regressed**

```bash
cargo run -p vox-cli --bin vox -- ci turso-import-guard --all
cargo run -p vox-cli --bin vox -- ci policy-allowlist-parity
```

Both should pass.

- [ ] **Step 8: Update the parity check to know about the merged source**

Re-read `crates/vox-cli/src/commands/ci/policy_allowlist_parity.rs`. The check now needs to consider that policy owners are AUTOMATICALLY in the allowlist — so the parity check's purpose shifts to "the YAML is parseable and consistent" rather than "the txt file mirrors the YAML". Update the run() function to verify:

1. The policy YAML parses cleanly.
2. Every `crates/<name>/` entry in the txt file is an EXISTING crate directory.
3. No txt entry duplicates a policy owner (warn / fail to keep the txt minimal).

```rust
// Add to run() before the "missing" check:
let crates_dir = root.join("crates");
for c in &allowlist_crates {
    if !crates_dir.join(c).is_dir() {
        return Err(anyhow!(
            "policy-allowlist-parity: docs/agents/turso-import-allowlist.txt lists \
             `crates/{c}/` but that directory does not exist."
        ));
    }
    if policy_crates.contains(c) {
        return Err(anyhow!(
            "policy-allowlist-parity: docs/agents/turso-import-allowlist.txt lists \
             `crates/{c}/` but `{c}` is already a policy owner — remove the txt entry, \
             the YAML is the source of truth."
        ));
    }
}
```

- [ ] **Step 9: Run all parity tests**

```bash
cargo test -p vox-cli policy_allowlist_parity
```

Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs \
        crates/vox-cli/src/commands/ci/policy_allowlist_parity.rs \
        docs/agents/turso-import-allowlist.txt \
        crates/vox-cli/Cargo.toml
git commit -m "refactor(ci): policy YAML is single source of truth for turso-import allowlist"
```

---

# PHASE 4 — Move pure row types into `vox-db-types`

`vox-db-types` exists as the L0 types crate but no consumer imports it directly — every caller pulls in the full `vox-db` (and turso). This phase makes the L0 split useful by:

1. Verifying which row/param types in `vox-db` have no turso/connection dependencies.
2. Moving those that don't into `vox-db-types`.
3. Migrating two pilot consumers (e.g. `vox-orchestrator-mcp`, `vox-skills`) to depend on `vox-db-types` directly when they only need types.

This phase is the largest. It can be split off into a follow-up PR if the executor judges the diff too big — that decision goes in the PR description.

### Task 4.1: Audit which `vox-db` types are pure data

**Files:**
- Read-only audit; output: `docs/superpowers/plans/data-audit/2026-05-08-vox-db-types-move-list.md`

- [ ] **Step 1: List every public type re-exported from `vox-db`**

```bash
rg -n '^pub use ' crates/vox-db/src/lib.rs > /tmp/vox-db-exports.txt
wc -l /tmp/vox-db-exports.txt
```

- [ ] **Step 2: For each type, classify**

Open `/tmp/vox-db-exports.txt`. For each `pub use X::Y;`, determine:
- **MOVE**: a struct/enum with only owned data (`String`, `i64`, `Vec<u8>`, `Option<…>`, `bool`, etc.) — no `turso::*`, no `tokio::*`, no `Connection`, no closures over `&VoxDb`.
- **KEEP**: anything that holds a connection, a tokio handle, an actor sender, or a function that takes `&VoxDb`.

Use `rg` to grep each name's source:

```bash
rg --files-with-matches "pub struct MemoryEntry" crates/vox-db/src
rg "turso::|VoxDb|Connection" crates/vox-db/src/<file>.rs
```

- [ ] **Step 3: Write the move-list document**

Create `docs/superpowers/plans/data-audit/2026-05-08-vox-db-types-move-list.md` with the format:

```markdown
# vox-db → vox-db-types move candidates

## Confirmed pure data (MOVE)
- `RegressionRow` — defined in `crates/vox-db/src/store/types/regression.rs:N` — no turso refs.
- `BuildHealthSummary` — defined in `crates/vox-db/src/...` — no turso refs.
- … (one line per candidate)

## Mixed / KEEP in vox-db
- `AutoMigrator` — owns `&VoxDb`. KEEP.
- `Migration` — references `migration.rs` runtime. KEEP.
- … (one line per non-candidate, with reason)

## Already in vox-db-types
- `MemoryEntry`, `LearnedPatternEntry`, `EmbeddingEntry`, ... (re-exported via `vox-db-types/src/store_types/rows_core.rs`)
```

- [ ] **Step 4: Commit the audit doc**

```bash
git add docs/superpowers/plans/data-audit/2026-05-08-vox-db-types-move-list.md
git commit -m "docs(vox-db): catalog pure-data types eligible to move to vox-db-types"
```

### Task 4.2: Move one type as a proof of concept

Pick the simplest MOVE candidate from the audit (e.g. a single-file struct). Move it; verify nothing breaks.

**Files:**
- Move: from `crates/vox-db/src/store/<some>.rs` (just the type) to `crates/vox-db-types/src/store_types/<dest>.rs`
- Modify: `crates/vox-db/src/lib.rs` (re-export from vox-db-types)
- Modify: `crates/vox-db/src/store/<some>.rs` (remove duplicate definition; add `use vox_db_types::TheType;`)

- [ ] **Step 1: Read the type definition and any helpers it carries**

If the type has a `Default` impl, helper functions, or `From` impls, those move with it (provided they don't reference `VoxDb`).

- [ ] **Step 2: Write the new file in vox-db-types**

Mirror the existing pattern in `vox-db-types/src/store_types/rows_core.rs`. If the new type is small, append to that file. If large, create a new submodule and `pub use` it from `store_types/mod.rs`.

- [ ] **Step 3: Add `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]`**

Phase 5 standardizes serde, so include it now to avoid revisiting.

- [ ] **Step 4: Update vox-db's source**

In the original file, replace the struct definition with `pub use vox_db_types::TheType;`. Run:

```bash
cargo check --workspace
```

Expected: clean.

- [ ] **Step 5: Update `vox-db/src/lib.rs` re-export**

The `pub use store::{... TheType ...}` line in `lib.rs` already re-exports from `store::*`. Since `store` now re-exports from `vox-db-types`, it should still work. Verify by running:

```bash
cargo check -p vox-db --tests
cargo test -p vox-db -- --nocapture
```

- [ ] **Step 6: Commit**

```bash
git add crates/vox-db/src crates/vox-db-types/src
git commit -m "refactor(vox-db): move <TheType> to vox-db-types (pilot)"
```

### Task 4.3: Bulk-move the rest of the MOVE list

Repeat Task 4.2's pattern for every type in the MOVE list. **Move in groups of 3-5 types per commit, not all at once** — a bisect-friendly history beats one mega-commit.

- [ ] **Step 1: Pick the next batch of 3-5 from the move list**

- [ ] **Step 2: For each, repeat 4.2 steps 2-5**

- [ ] **Step 3: After each batch, run**

```bash
cargo check --workspace
cargo test -p vox-db --lib
```

- [ ] **Step 4: Commit the batch**

```bash
git commit -am "refactor(vox-db): move <BatchName1>, <BatchName2>, … to vox-db-types"
```

- [ ] **Step 5: Repeat until move-list is empty**

### Task 4.4: Migrate two consumer crates to depend on `vox-db-types` directly

Pick two crates that ONLY consume row types (no `VoxDb` calls). Likely candidates: a CLI command that formats a row for output, or a serializer.

- [ ] **Step 1: Identify candidates**

```bash
# crates that import vox-db but never call it as a connection:
rg -l 'use vox_db::' crates/ | while read f; do
    if ! rg -q 'VoxDb::|\.store\(|\.query\(|\.execute\(' "$f"; then
        echo "$f — pure types consumer"
    fi
done
```

- [ ] **Step 2: Pick the simplest candidate (e.g. a printer/formatter file)**

- [ ] **Step 3: Update its Cargo.toml**

Replace `vox-db = { workspace = true }` with `vox-db-types = { workspace = true }` IF removing `vox-db` doesn't break the crate. (Some crates need both — only swap when the crate ONLY uses types.)

- [ ] **Step 4: Update imports**

```bash
# in the candidate crate's source tree:
rg -l 'use vox_db::' crates/<crate>/ | while read f; do
    sed -i.bak 's|use vox_db::|use vox_db_types::|g' "$f"
    rm "$f.bak"
done
```

`sed -i` syntax differs on macOS (`sed -i ''`) vs Linux/Windows. Use Edit tool for precise edits if any file fails to compile.

- [ ] **Step 5: Build & test**

```bash
cargo check -p <crate>
cargo test -p <crate>
```

If it fails because some types aren't yet moved to `vox-db-types`, EITHER move them (extend Phase 4.3) OR revert the swap and pick a different candidate.

- [ ] **Step 6: Commit per crate**

```bash
git commit -am "refactor(<crate>): depend on vox-db-types directly (pure-types consumer)"
```

- [ ] **Step 7: Repeat for second candidate**

### Task 4.5: Update `where-things-live.md`

- [ ] **Step 1: Open the file and update the table**

`docs/src/architecture/where-things-live.md` — add a row clarifying:

```markdown
| Add a pure-data DB row type | `crates/vox-db-types/src/store_types/` (NOT `vox-db`) |
```

If the row already exists, ensure it's clear. Add a paragraph after the table:

```markdown
> **L0/L1 split:** if your consumer only needs row/param TYPES (no async, no
> connection), depend on `vox-db-types` directly — not on `vox-db`. The full
> `vox-db` crate transitively pulls in `turso` and tokio.
```

- [ ] **Step 2: Commit**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(architecture): clarify vox-db vs vox-db-types decision"
```

---

# PHASE 5 — Standardize serde derives on row types

Today, derives are inconsistent across `rows_core.rs` and `rows_extended.rs` — some have `Serialize, Deserialize`, some don't, with no obvious pattern. We pick one rule and enforce it mechanically.

**Decision:** every public `*Row` and `*Entry` struct in `vox-db-types` derives `Debug`, `Clone`, `serde::Serialize`, `serde::Deserialize`. Cost: zero (derives are zero-overhead at runtime; compile-time is negligible). Benefit: any consumer can serialize without writing wrapper types.

### Task 5.1: Failing test for serde derive uniformity

**Files:**
- Create: `crates/vox-db-types/tests/serde_uniformity.rs`

- [ ] **Step 1: Write a compile-test that asserts a sample of types implement Serialize+Deserialize**

```rust
//! Compile-time assertion: every public row/entry type in `vox-db-types`
//! must derive `Serialize` and `Deserialize`.
//!
//! Add new types to this file as they are introduced. The static_assertions
//! crate is preferred but a plain `fn assert_serde<T: Serialize + DeserializeOwned>(){}`
//! works.

use serde::Serialize;
use serde::de::DeserializeOwned;
use vox_db_types::*;

fn assert_serde<T: Serialize + DeserializeOwned>() {}

#[test]
fn all_row_types_implement_serde() {
    assert_serde::<MemoryEntry>();
    assert_serde::<LearnedPatternEntry>();
    assert_serde::<EmbeddingEntry>();
    assert_serde::<ExecutionEntry>();
    assert_serde::<ScheduledEntry>();
    assert_serde::<ComponentEntry>();
    assert_serde::<BehaviorEventEntry>();
    assert_serde::<CommandFrequencyEntry>();
    assert_serde::<TrainingPair>();
    assert_serde::<UserEntry>();
    assert_serde::<AgentDefEntry>();
    assert_serde::<SnippetEntry>();
    assert_serde::<PackageSearchResult>();
    assert_serde::<ArtifactEntry>();
    assert_serde::<SkillManifestEntry>();
    assert_serde::<KnowledgeNodeSummary>();
    assert_serde::<BuilderSessionEntry>();
    assert_serde::<SessionTurnEntry>();
    assert_serde::<TypedStreamEventEntry>();
    assert_serde::<ReviewEntry>();
    assert_serde::<CodexChangeLogEntry>();
    // …extend with every type from rows_core.rs and rows_extended.rs
}
```

To enumerate the full list, run:

```bash
rg --no-heading -o 'pub struct ([A-Z][A-Za-z0-9]*(?:Row|Entry|Result|Summary|Pair|Report|Rollup|Snapshot|Profile|Job))' crates/vox-db-types/src
```

Add every match to the test.

- [ ] **Step 2: Run — should fail**

```bash
cargo test -p vox-db-types --test serde_uniformity
```

Expected: FAIL — at least `ExecutionEntry`, `ScheduledEntry`, `ComponentEntry`, `EmbeddingEntry`, `BehaviorEventEntry`, `CommandFrequencyEntry`, etc. lack the derives.

### Task 5.2: Add the missing derives

**Files:**
- Modify: `crates/vox-db-types/src/store_types/rows_core.rs`
- Modify: `crates/vox-db-types/src/store_types/rows_extended.rs`
- Modify: any other type files surfaced by Task 5.1's grep

- [ ] **Step 1: For each failing type, add the derives**

Pattern: replace `#[derive(Debug, Clone)]` with `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]`.

Use Edit tool individually rather than a global sed, because some types have additional derives (`PartialEq`, `Eq`, `Hash`) that must be preserved.

Example (`ExecutionEntry`, `rows_core.rs:1-16`):

```rust
/// One row from `execution_log`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionEntry {
```

- [ ] **Step 2: Run the test**

```bash
cargo test -p vox-db-types --test serde_uniformity
```

Expected: PASS.

- [ ] **Step 3: Run the broader workspace check**

```bash
cargo check --workspace
```

If a downstream consumer breaks because adding `Serialize` introduces a name conflict (very rare), investigate and either rename or add `#[serde(rename = ...)]`.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-db-types/src crates/vox-db-types/tests
git commit -m "feat(vox-db-types): derive Serialize/Deserialize on all row/entry types"
```

### Task 5.3: Lint to catch new types missing the derives

We can't reliably introspect derives at compile time without a proc-macro. Instead, add a lightweight grep-based CI check.

**Files:**
- Create: `crates/vox-cli/src/commands/ci/row_serde_lint.rs`
- Wire as in Phase 2.

- [ ] **Step 1: Write the lint**

```rust
//! Verifies every public struct in `vox-db-types/src/store_types/` whose name
//! ends in Row/Entry/Result/Summary/Pair/Report/Rollup/Snapshot/Profile/Job
//! derives both `Serialize` and `Deserialize`.

use anyhow::{Result, anyhow};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

pub fn run(root: &Path) -> Result<()> {
    let dir = root.join("crates/vox-db-types/src/store_types");
    let mut violations = Vec::new();
    walk(&dir, &mut |path| {
        if path.extension().and_then(|e| e.to_str()) != Some("rs") { return Ok(()); }
        let body = fs::read_to_string(path)?;
        check_file(path, &body, &mut violations);
        Ok(())
    })?;
    if !violations.is_empty() {
        return Err(anyhow!(
            "row-serde-lint: {} type(s) missing serde derives:\n{}",
            violations.len(),
            violations.join("\n"),
        ));
    }
    println!("row-serde-lint OK");
    Ok(())
}

fn check_file(path: &Path, body: &str, out: &mut Vec<String>) {
    let struct_re = Regex::new(
        r"(?ms)#\[derive\(([^)]*)\)\]\s*pub\s+struct\s+([A-Z][A-Za-z0-9]*(?:Row|Entry|Result|Summary|Pair|Report|Rollup|Snapshot|Profile|Job))\b"
    ).unwrap();
    for cap in struct_re.captures_iter(body) {
        let derives = cap.get(1).unwrap().as_str();
        let name = cap.get(2).unwrap().as_str();
        let has_ser = derives.contains("Serialize");
        let has_de = derives.contains("Deserialize");
        if !(has_ser && has_de) {
            out.push(format!(
                "  {}: struct `{}` missing {}{}{}",
                path.display(), name,
                if !has_ser { "Serialize" } else { "" },
                if !has_ser && !has_de { ", " } else { "" },
                if !has_de { "Deserialize" } else { "" },
            ));
        }
    }
}

fn walk(dir: &Path, f: &mut dyn FnMut(&Path) -> Result<()>) -> Result<()> {
    if !dir.is_dir() { return Ok(()); }
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() { walk(&p, f)?; }
        else { f(&p)?; }
    }
    Ok(())
}
```

- [ ] **Step 2: Wire as a subcommand**

Mirror Phase 2 Task 2.2 Steps 1-5 (variant + dispatch + module + registry).

- [ ] **Step 3: Run on the workspace**

```bash
cargo run -p vox-cli --bin vox -- ci row-serde-lint
```

Expected: PASS.

- [ ] **Step 4: Add integration test**

Mirror `db_schema_coverage_integration.rs`. Place at `crates/vox-cli/tests/row_serde_lint_integration.rs`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/row_serde_lint.rs \
        crates/vox-cli/src/commands/ci/cmd_enums.rs \
        crates/vox-cli/src/commands/ci/run_body.rs \
        crates/vox-cli/src/commands/ci/mod.rs \
        crates/vox-cli/tests/row_serde_lint_integration.rs \
        contracts/cli/command-registry.yaml
git commit -m "feat(ci): row-serde-lint catches new row types missing Serialize/Deserialize"
```

---

# PHASE 6 — Add `From`/`TryFrom` row↔domain bridges

Today: callers manually destructure `SessionRow.agent_id: String` into application objects everywhere. We add typed conversions for the 3-4 most-touched pairs and migrate one or two callers as a pattern proof.

### Task 6.1: Identify the 3-4 hottest row→domain pairs

- [ ] **Step 1: Find the most-referenced rows**

```bash
for ty in SessionRow AgentDefEntry MemoryEntry SessionEventRow SkillExecutionRow PlanSessionRow A2AMessageRow; do
    count=$(rg -l --type rust "\b$ty\b" crates/ | wc -l)
    echo "$count $ty"
done | sort -rn
```

The top 3-4 by file count are the targets.

- [ ] **Step 2: For each, find the "domain" type that consumers construct from the row**

```bash
rg -B2 -A8 'fn .*from.*SessionRow|impl From<SessionRow>' crates/
```

If no domain type exists, skip that row (no bridge needed). If one does (e.g. `Session` in `vox-orchestrator-types`), it's the bridge target.

- [ ] **Step 3: Document picks**

Append to `docs/superpowers/plans/data-audit/2026-05-08-vox-db-types-move-list.md`:

```markdown
## Row↔Domain bridges to add (Phase 6)
- `vox_db_types::SessionRow` → `vox_orchestrator_types::Session`
- `vox_db_types::AgentDefEntry` → `vox_orchestrator_types::AgentDefinition`
- … (3-4 pairs)
```

### Task 6.2: Implement the bridges

For each pair, the bridge lives in `vox-db-types` (NOT in `vox-orchestrator-types` — `vox-db-types` already knows about row shapes; orchestrator-types should not have to know about DB rows). Use `TryFrom` (not `From`) when conversion can fail (e.g. enum parsing, malformed JSON).

**Files:**
- Modify: `crates/vox-db-types/src/store_types/conversions.rs` (create)
- Modify: `crates/vox-db-types/src/store_types/mod.rs` (add module)
- Modify: `crates/vox-db-types/Cargo.toml` (add `vox-orchestrator-types` dep — verify no layer-rule violation first)

- [ ] **Step 1: Check the layer rule**

```bash
rg -A20 'name = "vox-db-types"' docs/src/architecture/layers.toml
```

If `vox-db-types` is at a strictly LOWER layer than `vox-orchestrator-types`, depending up is forbidden — instead, put the conversions in `vox-orchestrator-types` (which depends on `vox-db-types` for the row types) or in a third crate. **STOP and check before adding the dep.**

- [ ] **Step 2: If layers permit, create the conversions module**

`crates/vox-db-types/src/store_types/conversions.rs`:

```rust
//! Bridges between DB row types (this crate) and domain types in upper layers.

use crate::store_types::SessionRow;
// (other use statements as needed)

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("session id {0:?} did not match any known prefix")]
    InvalidSessionId(String),
    #[error("agent id {0:?} did not parse: {1}")]
    InvalidAgentId(String, String),
    // …
}

impl SessionRow {
    /// Returns a typed [`vox_orchestrator_types::Session`] view of this row.
    pub fn as_session(&self) -> Result<vox_orchestrator_types::Session, BridgeError> {
        Ok(vox_orchestrator_types::Session {
            id: self.id.clone(),
            agent_id: self.agent_id.clone(),
            // …other fields
        })
    }
}
```

If `vox_orchestrator_types::Session` does NOT yet exist as a domain type (only the runtime AgentId/TaskId newtypes do), STOP — you're inventing a domain type. That's out of scope for this phase. Skip this pair and pick another.

- [ ] **Step 3: Add a unit test for each bridge**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_row_to_domain_session() {
        let row = SessionRow {
            id: "S-12345".into(),
            agent_id: "agent-abc".into(),
            // …populate
        };
        let session = row.as_session().expect("valid row");
        assert_eq!(session.id, "S-12345");
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p vox-db-types
```

- [ ] **Step 5: Migrate ONE caller**

Find one place where `SessionRow.agent_id` is destructured and switch it to `.as_session()`. This is the proof-of-pattern; not a sweep.

- [ ] **Step 6: Commit per pair**

```bash
git commit -am "feat(vox-db-types): add SessionRow → Session bridge + first caller migration"
```

- [ ] **Step 7: Repeat for the other 2-3 pairs**

If any pair requires inventing a domain type, defer it — note in the PR description.

---

# PHASE 7 — Typed string-ID newtypes for DB rows

The orchestrator has `TaskId(u64)`, `AgentId(u64)` newtypes; DB rows use raw `String`. We don't unify them in this PR (too invasive), but we DO introduce DB-side string newtypes that signal intent: `DbAgentId(String)`, `DbSessionId(String)`, `DbTaskId(String)`. Consumers can then write `fn lookup(id: DbAgentId)` instead of `fn lookup(id: String)` — typo-proof and self-documenting.

### Task 7.1: Add the newtypes (no row changes yet)

**Files:**
- Create: `crates/vox-db-types/src/ids.rs`
- Modify: `crates/vox-db-types/src/lib.rs` (re-export)

- [ ] **Step 1: Write the newtypes**

```rust
//! String-typed ID newtypes for DB rows. These wrap stringly-typed IDs from
//! libSQL columns (UUIDs, hashes, human-readable IDs) without committing to
//! a specific format. Pair these with the orchestrator's `TaskId(u64)` only
//! at the orchestrator boundary — they live in different layers.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! string_id {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(s: impl Into<String>) -> Self { Self(s.into()) }
            pub fn as_str(&self) -> &str { &self.0 }
            pub fn into_string(self) -> String { self.0 }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self { Self(s) }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self { Self(s.to_string()) }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str { &self.0 }
        }
    };
}

string_id!(/// Stringly-typed agent ID as stored in DB rows.
    DbAgentId);
string_id!(/// Stringly-typed session ID as stored in DB rows.
    DbSessionId);
string_id!(/// Stringly-typed task ID as stored in DB rows.
    DbTaskId);
string_id!(/// Stringly-typed correlation ID as stored in DB rows.
    DbCorrelationId);
string_id!(/// Stringly-typed user ID as stored in DB rows.
    DbUserId);
string_id!(/// Stringly-typed plan-session ID as stored in DB rows.
    DbPlanSessionId);
```

- [ ] **Step 2: Re-export from `lib.rs`**

In `crates/vox-db-types/src/lib.rs`:

```rust
pub mod ids;
pub use ids::{DbAgentId, DbSessionId, DbTaskId, DbCorrelationId, DbUserId, DbPlanSessionId};
```

- [ ] **Step 3: Re-export from `vox-db/src/lib.rs`**

Append a re-export so existing `use vox_db::DbAgentId` works:

```rust
pub use vox_db_types::{DbAgentId, DbSessionId, DbTaskId, DbCorrelationId, DbUserId, DbPlanSessionId};
```

- [ ] **Step 4: Add unit tests**

`crates/vox-db-types/src/ids.rs` bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_serde_json() {
        let id = DbAgentId::new("agent-42");
        let s = serde_json::to_string(&id).unwrap();
        assert_eq!(s, "\"agent-42\"");
        let back: DbAgentId = serde_json::from_str(&s).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn display_matches_inner_string() {
        let id = DbSessionId::new("S-001");
        assert_eq!(format!("{id}"), "S-001");
    }

    #[test]
    fn distinct_types_do_not_unify() {
        // Compile-time test: this should NOT compile if uncommented:
        // let a: DbAgentId = DbSessionId::new("x");
        let a = DbAgentId::new("a");
        let b = DbSessionId::new("a");
        // Same string, different types.
        assert_eq!(a.as_str(), b.as_str());
    }
}
```

- [ ] **Step 5: Run**

```bash
cargo test -p vox-db-types ids
cargo check --workspace
```

If `serde_json` isn't a dev-dep of vox-db-types, add it under `[dev-dependencies]`.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-db-types/src/ids.rs crates/vox-db-types/src/lib.rs \
        crates/vox-db/src/lib.rs crates/vox-db-types/Cargo.toml
git commit -m "feat(vox-db-types): add Db<Entity>Id string newtypes"
```

### Task 7.2: Migrate ONE row to use the newtype as a proof of pattern

**Do not migrate all rows in this PR.** That's a sweeping refactor. Pick the most-touched row (likely `MemoryEntry` or `SessionRow`) and switch its `agent_id: String` to `agent_id: DbAgentId`.

**Files:**
- Modify: `crates/vox-db-types/src/store_types/rows_core.rs` (one struct's field)
- Modify: every caller that destructures the field

- [ ] **Step 1: Pick the row**

E.g. `MemoryEntry`. Find every caller:

```bash
rg -n 'MemoryEntry' crates/
```

- [ ] **Step 2: Change the field type**

```rust
pub struct MemoryEntry {
    pub id: i64,
    pub agent_id: DbAgentId,    // was: String
    pub session_id: DbSessionId, // was: String
    pub memory_type: String,
    // …
}
```

- [ ] **Step 3: Update each caller**

Run `cargo check --workspace` and follow the errors. Typical fixes:
- `entry.agent_id` (was `String`) is now `DbAgentId` — callers reading `&entry.agent_id` may need `entry.agent_id.as_str()`.
- Callers constructing `MemoryEntry { agent_id: "x".to_string() }` need `agent_id: DbAgentId::new("x")` or `agent_id: "x".into()`.

Use Edit tool per file; do not bulk-sed.

- [ ] **Step 4: Update store ops in vox-db**

The mapping `row → MemoryEntry` in `vox-db/src/store/ops_*.rs` needs `DbAgentId::new(row.get(...))`. Find:

```bash
rg -n 'MemoryEntry\s*\{' crates/vox-db/src
```

- [ ] **Step 5: Build and test**

```bash
cargo check --workspace
cargo test -p vox-db --lib
cargo test -p vox-db-types
```

- [ ] **Step 6: Commit**

```bash
git commit -am "refactor(vox-db-types): MemoryEntry uses DbAgentId/DbSessionId newtypes"
```

### Task 7.3: Add a lint that flags new `String`-typed `*_id` fields

Future protection: catch new row fields named `agent_id`, `session_id`, etc. typed as `String` rather than the newtype.

**Files:**
- Create: `crates/vox-cli/src/commands/ci/string_id_lint.rs`

- [ ] **Step 1: Write the lint**

```rust
//! Flags new `*_id: String` fields in `crates/vox-db-types/src/store_types/`
//! that should use one of the `Db<Entity>Id` newtypes.

use anyhow::{Result, anyhow};
use regex::Regex;
use std::fs;
use std::path::Path;

const MAPPED_IDS: &[(&str, &str)] = &[
    ("agent_id",          "DbAgentId"),
    ("session_id",        "DbSessionId"),
    ("task_id",           "DbTaskId"),
    ("correlation_id",    "DbCorrelationId"),
    ("user_id",           "DbUserId"),
    ("plan_session_id",   "DbPlanSessionId"),
];

pub fn run(root: &Path) -> Result<()> {
    let dir = root.join("crates/vox-db-types/src/store_types");
    let mut violations = Vec::new();
    walk(&dir, &mut |path| {
        if path.extension().and_then(|e| e.to_str()) != Some("rs") { return Ok(()); }
        let body = fs::read_to_string(path)?;
        for (field, ty) in MAPPED_IDS {
            let re = Regex::new(&format!(r"\bpub\s+{field}\s*:\s*(?:Option<)?String")).unwrap();
            for m in re.find_iter(&body) {
                let line_no = body[..m.start()].lines().count() + 1;
                violations.push(format!(
                    "  {}:{}  field `{}: String` should use `{}`",
                    path.display(), line_no, field, ty,
                ));
            }
        }
        Ok(())
    })?;
    if !violations.is_empty() {
        return Err(anyhow!(
            "string-id-lint: {} stringly-typed ID field(s) where a newtype exists:\n{}",
            violations.len(),
            violations.join("\n"),
        ));
    }
    println!("string-id-lint OK");
    Ok(())
}

fn walk(dir: &Path, f: &mut dyn FnMut(&Path) -> Result<()>) -> Result<()> {
    if !dir.is_dir() { return Ok(()); }
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() { walk(&p, f)?; }
        else { f(&p)?; }
    }
    Ok(())
}
```

- [ ] **Step 2: Wire the subcommand (mirror Phase 2.2)**

- [ ] **Step 3: Run it**

```bash
cargo run -p vox-cli --bin vox -- ci string-id-lint
```

EXPECTED: this WILL fail because Task 7.2 only migrated one row. **That's fine** — instead of fixing every row in this PR, the lint should run in `--report` mode, NOT fail CI yet.

Add a `--report-only` flag:

```rust
pub fn run(root: &Path, report_only: bool) -> Result<()> {
    // …
    if !violations.is_empty() {
        let msg = format!(
            "string-id-lint: {} stringly-typed ID field(s):\n{}",
            violations.len(), violations.join("\n"),
        );
        if report_only {
            eprintln!("WARN: {msg}");
            println!("string-id-lint REPORT-ONLY ({} findings)", violations.len());
            return Ok(());
        } else {
            return Err(anyhow!(msg));
        }
    }
    println!("string-id-lint OK");
    Ok(())
}
```

Default to `--report-only true` for now; flip to false in a follow-up PR after the sweep lands.

- [ ] **Step 4: Commit**

```bash
git commit -am "feat(ci): string-id-lint (report-only) for stringly-typed *_id row fields"
```

---

# PHASE 8 — Document operational JSON state

[`crates/vox-cli/src/process_supervision.rs`](../../../../crates/vox-cli/src/process_supervision.rs) writes JSON state files for process management. [`crates/vox-orchestrator/src/orchestrator/persistence/lifecycle.rs`](../../../../crates/vox-orchestrator/src/orchestrator/persistence/lifecycle.rs) serializes context store snapshots. These are NOT in the DB. Add comments explaining why so future readers don't ask.

### Task 8.1: Add module-level docs

**Files:**
- Modify: `crates/vox-cli/src/process_supervision.rs` (top of file)
- Modify: `crates/vox-orchestrator/src/orchestrator/persistence/lifecycle.rs` (top of file)

- [ ] **Step 1: Read both file headers**

```bash
sed -n '1,30p' crates/vox-cli/src/process_supervision.rs
sed -n '1,30p' crates/vox-orchestrator/src/orchestrator/persistence/lifecycle.rs
```

- [ ] **Step 2: Add or extend the module doc on `process_supervision.rs`**

If the file lacks a `//!` doc comment, add at top:

```rust
//! Process supervision state files (`<base>.state.json`) — written under
//! `.vox/process-supervision/`.
//!
//! **Why JSON, not the DB?** These files describe operational ephemera (PID,
//! socket path, last heartbeat) for processes the CLI manages. They:
//!   * are written before the DB connection exists (chicken/egg with
//!     orchestrator startup),
//!   * are stale-by-design when the process exits (no migration concerns),
//!   * are not user data and have no cross-machine value.
//! Tier-D (cache) per `contracts/db/data-storage-policy.v1.yaml`.
```

If a doc already exists, append the "Why JSON" paragraph.

- [ ] **Step 3: Same for `lifecycle.rs`**

```rust
//! Orchestrator context-store snapshot persistence — JSON, not the DB.
//!
//! The in-memory `ContextStore` (`vox-orchestrator/src/context/mod.rs`) is
//! serialized to JSON for crash-recovery snapshots. It is **not** in the DB
//! because:
//!   * the snapshot is written on a "fast" path during shutdown, before any
//!     async runtime guarantee,
//!   * the data is opaque to other consumers (orchestrator-internal),
//!   * keeping it out of the DB avoids schema churn for orchestrator changes.
//! If this changes (e.g. cross-process visibility is needed), this is the
//! place to write a `vox_db::ops_orchestrator_context` module instead.
```

- [ ] **Step 4: Build (sanity)**

```bash
cargo check -p vox-cli -p vox-orchestrator
```

- [ ] **Step 5: Commit**

```bash
git commit -am "docs(persistence): explain why operational JSON state is not in the DB"
```

---

# PHASE 9 — Architecture documentation updates

### Task 9.1: Update ADR-004

**Files:**
- Modify: `docs/src/adr/004-codex-arca-turso-ssot.md`

- [ ] **Step 1: Read the current ADR**

```bash
sed -n '1,200p' docs/src/adr/004-codex-arca-turso-ssot.md
```

- [ ] **Step 2: Add a new "Status update — 2026-05" section**

Append (do NOT rewrite history):

```markdown
## Status update — 2026-05

### Sanctioned satellites (libSQL files outside `vox.db`)

The "Turso-only" rule does not mean "single DB file." It means "every relational
store uses libSQL/Turso, with the schema either in `vox-db`'s `SCHEMA_FRAGMENTS`
manifest or in an explicitly-listed sanctioned satellite."

Current sanctioned satellites:

| Crate | DB file | Reason | Owner |
|---|---|---|---|
| `vox-secrets` | `.vox/clavis_vault.db` | Blast-radius isolation: secrets must never share a process-level connection with user-data Codex. | Security |
| `vox-package` | `.vox_modules/local_store.db` | Transitional; folded away by M-67. | Package |

The list above is mirrored mechanically in
[`contracts/db/data-storage-policy.v1.yaml`](../../../../contracts/db/data-storage-policy.v1.yaml)
(`tiers.a_relational.{owners, allow_direct_access, temporary_exceptions}`).
Three CI checks enforce no further drift:

* `vox ci db-schema-coverage` — every `CREATE TABLE` lives in an owner crate.
* `vox ci policy-allowlist-parity` — txt allowlist agrees with policy YAML.
* `vox ci turso-import-guard` — built-in prefixes auto-derived from policy YAML.

### What is NOT a satellite

* Operational JSON state (`.vox/process-supervision/*.state.json`, orchestrator
  context snapshots) — Tier D cache. See file headers for rationale.
* In-process `Arc<Mutex<HashMap>>` registries (rate limit buckets, broadcast
  subscriptions, per-request receipts) — ephemeral by design.
```

- [ ] **Step 3: Commit**

```bash
git add docs/src/adr/004-codex-arca-turso-ssot.md
git commit -m "docs(adr-004): document sanctioned libSQL satellites + new CI checks"
```

### Task 9.2: Update `where-things-live.md`

**Files:**
- Modify: `docs/src/architecture/where-things-live.md`

- [ ] **Step 1: Add an entry for the new CI checks**

Find the "Common tasks → exact path" table and add:

```markdown
| Add a new ci/db guard | `crates/vox-cli/src/commands/ci/<name>.rs` + register in `cmd_enums.rs` and `run_body.rs`. Mirror `db_schema_coverage.rs`. |
| Add a Db<Entity>Id newtype | `crates/vox-db-types/src/ids.rs` (use the `string_id!` macro). |
```

- [ ] **Step 2: Commit**

```bash
git commit -am "docs(architecture): list new ci/db guard task + Db<Entity>Id task"
```

### Task 9.3: Update `database-nomenclature.md` if present

**Files:**
- Modify: `docs/agents/database-nomenclature.md` (only if it exists)

- [ ] **Step 1: Check for existence**

```bash
test -f docs/agents/database-nomenclature.md && echo OK || echo MISSING
```

If MISSING, skip this task.

- [ ] **Step 2: Add a "Pure types vs facade" section**

Add before the closing of the file:

```markdown
## When to depend on `vox-db-types` vs `vox-db`

| You need... | Depend on |
|---|---|
| Row/param structs only (no async, no connection) | `vox-db-types` |
| `VoxDb` connection, `.store(...)`, queries | `vox-db` |
| The `DbAgentId`/`DbSessionId`/etc. newtypes | `vox-db-types` (re-exported from `vox-db`) |

The mechanical lint `vox ci row-serde-lint` enforces that every row type derives
`Serialize` + `Deserialize`; `vox ci string-id-lint` (report-only) flags new
stringly-typed ID fields where a newtype exists.
```

- [ ] **Step 3: Commit**

```bash
git commit -am "docs(agents): document vox-db-types vs vox-db decision"
```

---

# PHASE 10 — Final verification & PR finalization

### Task 10.1: Full build & test sweep

- [ ] **Step 1: Workspace check**

```bash
cargo check --workspace --all-targets
```

Expected: clean.

- [ ] **Step 2: Workspace clippy**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

If clippy fails on code we did NOT touch, document and skip; don't fix unrelated lints in this PR.

- [ ] **Step 3: Workspace tests**

```bash
cargo test --workspace
```

Expected: all green. Capture the runtime; if any test newly takes >2x baseline, investigate.

- [ ] **Step 4: Run every guard**

```bash
cargo run -p vox-cli --bin vox -- ci turso-import-guard --all
cargo run -p vox-cli --bin vox -- ci policy-allowlist-parity
cargo run -p vox-cli --bin vox -- ci db-schema-coverage
cargo run -p vox-cli --bin vox -- ci row-serde-lint
cargo run -p vox-cli --bin vox -- ci string-id-lint
cargo run -p vox-cli --bin vox -- ci data-storage-guard
```

Expected: all `OK`. If `string-id-lint` reports findings, that's expected (report-only mode in this PR).

- [ ] **Step 5: Run vox-arch-check**

```bash
cargo run -p vox-arch-check
```

Expected: clean. If a new dep introduced a layer inversion, either revert (Phase 6 dep) or add an entry to `[[known_inversions]]` in `layers.toml` with a `reason`.

### Task 10.2: Regenerate auto-generated docs

Per user memory `feedback_auto_generated_docs.md`: NEVER hand-edit auto-generated files. Re-run their generators.

- [ ] **Step 1: Identify which generators to run**

```bash
rg -l 'auto-generated|DO NOT EDIT' docs/src/SUMMARY.md docs/src/architecture/architecture-index.md docs/feed.xml 2>/dev/null
```

- [ ] **Step 2: Run regenerators**

If the project provides `vox doc regen` or similar, run it. Otherwise:

```bash
cargo run -p vox-doc-pipeline -- regenerate
```

(Adjust to actual entrypoint — `rg -n 'fn main' crates/vox-doc-pipeline/src` to locate.)

- [ ] **Step 3: Commit any changes**

```bash
git status
# review carefully
git add -A
git commit -m "chore(docs): regenerate auto-generated indices"
```

### Task 10.3: Review every commit on the branch

- [ ] **Step 1: List the branch's commits**

```bash
git log --oneline main..HEAD
```

Expected: a coherent sequence (~15-25 commits) — each commit-message-line should make sense to a reviewer reading the PR.

- [ ] **Step 2: Squash-fix any accidental "wip" or noise commits**

If you see "fix typo" / "wip" commits, interactive rebase them away — but DO NOT use `git rebase -i` (per Bash tool docs: never use `-i` flags). Instead, use `git reset --soft <hash>` then re-commit, OR leave them — the PR description explains the story.

- [ ] **Step 3: Ensure no auto-generated doc was hand-edited**

```bash
git log --name-only main..HEAD -- 'docs/src/architecture/architecture-index.md' 'docs/src/SUMMARY.md' 'docs/feed.xml'
```

If any commit is a hand-edit of these files (without an accompanying generator-run commit), revert it.

### Task 10.4: Run the integration test once more in --release

The CI runner runs in release; some failures only show there.

- [ ] **Step 1: Build release**

```bash
cargo build --workspace --release
```

- [ ] **Step 2: Run the new integration tests in release**

```bash
cargo test --release -p vox-cli --test policy_allowlist_parity_integration \
                                  --test db_schema_coverage_integration \
                                  --test turso_import_guard_integration \
                                  --test row_serde_lint_integration
```

Expected: all PASS.

### Task 10.5: Open the PR

- [ ] **Step 1: Push the branch**

```bash
git push -u origin cc_bdesktop2/magical-thompson-3fc3a4
```

- [ ] **Step 2: Open PR**

```bash
gh pr create --title "Vox-DB & memory audit: close policy↔enforcement gap, real L0 types, typed IDs" --body "$(cat <<'EOF'
## Summary

- Closes the policy↔enforcement gap for Turso usage: `data-storage-policy.v1.yaml` is now the single source of truth for the turso-import-guard allowlist.
- Adds three new CI checks: `policy-allowlist-parity`, `db-schema-coverage`, `row-serde-lint` (and a report-only `string-id-lint`).
- Standardizes `Serialize`/`Deserialize` derives on every row/entry type in `vox-db-types`.
- Moves pure-data row types from `vox-db` into `vox-db-types` and migrates two consumer crates to depend on `vox-db-types` directly — the L0/L1 split is finally load-bearing.
- Introduces `Db<Entity>Id` string newtypes (`DbAgentId`, `DbSessionId`, `DbTaskId`, `DbCorrelationId`, `DbUserId`, `DbPlanSessionId`); migrates `MemoryEntry` as a pattern proof. Wholesale row migration deferred to a follow-up PR; report-only lint flags remaining stringly-typed fields.
- Adds `From`/`TryFrom` row↔domain bridges for the 3-4 hottest pairs.
- Documents the operational JSON state (process supervision, orchestrator snapshots) as Tier D cache by design.
- Updates ADR-004 with the sanctioned-satellite policy + new CI checks; updates `where-things-live.md` and `database-nomenclature.md` accordingly.

## Why one PR

Per the audit recommendation, all six work orders were grouped into one PR for atomic review of the consistency story. The diff is large (~?? files) but each commit is self-contained and bisect-friendly.

## Test plan

- [ ] `cargo test --workspace` green on CI
- [ ] `cargo run -p vox-cli -- ci turso-import-guard --all` exits 0
- [ ] `cargo run -p vox-cli -- ci policy-allowlist-parity` exits 0
- [ ] `cargo run -p vox-cli -- ci db-schema-coverage` exits 0
- [ ] `cargo run -p vox-cli -- ci row-serde-lint` exits 0
- [ ] `cargo run -p vox-arch-check` clean
- [ ] Manual: confirm `MemoryEntry::agent_id` round-trips through serde with the new newtype
- [ ] Manual: confirm a hypothetical new crate adding `CREATE TABLE foo` is rejected by `db-schema-coverage`

## Out of scope (follow-ups)

- Wholesale migration of remaining row types from `String` to `Db<Entity>Id` newtypes (covered by report-only lint).
- Wholesale row→domain bridges beyond the 3-4 pilot pairs.
- Flipping `string-id-lint` from `--report-only` to fail-closed.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 3: Verify PR rendered correctly**

```bash
gh pr view --web
```

- [ ] **Step 4: Return the PR URL to the user**

---

## Appendix A — Files this PR touches (estimated)

**Created:**
- `crates/vox-cli/src/commands/ci/policy_allowlist_parity.rs`
- `crates/vox-cli/src/commands/ci/db_schema_coverage.rs`
- `crates/vox-cli/src/commands/ci/row_serde_lint.rs`
- `crates/vox-cli/src/commands/ci/string_id_lint.rs`
- `crates/vox-cli/tests/policy_allowlist_parity_integration.rs`
- `crates/vox-cli/tests/db_schema_coverage_integration.rs`
- `crates/vox-cli/tests/row_serde_lint_integration.rs`
- `crates/vox-cli/tests/fixtures/db-schema-coverage/known-tables.txt`
- `crates/vox-db-types/src/ids.rs`
- `crates/vox-db-types/src/store_types/conversions.rs`
- `crates/vox-db-types/tests/serde_uniformity.rs`
- `scripts/extract_table_names.vox`
- `docs/superpowers/plans/data-audit/2026-05-08-vox-db-audit-baseline.md`
- `docs/superpowers/plans/data-audit/2026-05-08-vox-db-types-move-list.md`

**Modified:**
- `crates/vox-cli/src/commands/ci/cmd_enums.rs` (4 new variants)
- `crates/vox-cli/src/commands/ci/run_body.rs` (4 new dispatches)
- `crates/vox-cli/src/commands/ci/mod.rs` (4 new module decls)
- `crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs` (`load_turso_import_allowlist` reads policy YAML)
- `crates/vox-cli/Cargo.toml` (`tempfile`, `serde_yaml` deps if needed)
- `crates/vox-db/src/lib.rs` (re-export new ids + relocated types)
- `crates/vox-db-types/src/lib.rs` (re-export ids module)
- `crates/vox-db-types/src/store_types/rows_core.rs` (serde derives + DbAgentId migration)
- `crates/vox-db-types/src/store_types/rows_extended.rs` (serde derives)
- `crates/vox-db-types/Cargo.toml` (potentially `vox-orchestrator-types` if layers permit)
- `crates/vox-cli/src/process_supervision.rs` (module doc)
- `crates/vox-orchestrator/src/orchestrator/persistence/lifecycle.rs` (module doc)
- `docs/src/adr/004-codex-arca-turso-ssot.md` (status update section)
- `docs/src/architecture/where-things-live.md` (new task entries)
- `docs/agents/turso-import-allowlist.txt` (prune duplicates with policy)
- `docs/agents/database-nomenclature.md` (new section)
- `contracts/cli/command-registry.yaml` (4 new entries)
- 2-N rows in `crates/vox-db-types/src/store_types/` (relocated from vox-db)
- 2 consumer crates' `Cargo.toml` and source (depend on `vox-db-types` directly)
- 1 row's caller sites (MemoryEntry → DbAgentId migration)

**Auto-regenerated (do not hand-edit):**
- `docs/src/SUMMARY.md`
- `docs/src/architecture/architecture-index.md`
- `docs/feed.xml`
- `contracts/cli/command-registry.yaml`'s generated index, if any

---

## Appendix B — Commit cadence (target)

A clean PR has ~15-25 commits in this shape:

1. `docs(plan): baseline notes for vox-db audit PR`
2. `chore(db-audit): snapshot known CREATE TABLE inventory for coverage check`
3. `fix(turso-guard): align allowlist with data-storage-policy (vox-secrets owner)`
4. `feat(ci): add policy-allowlist-parity check`
5. `chore(ci): chain policy-allowlist-parity into umbrella guard run`
6. `feat(ci): db-schema-coverage check`
7. `chore(ci): chain db-schema-coverage into umbrella guard run`
8. `refactor(ci): policy YAML is single source of truth for turso-import allowlist`
9. `docs(vox-db): catalog pure-data types eligible to move to vox-db-types`
10. `refactor(vox-db): move <FirstType> to vox-db-types (pilot)`
11. `refactor(vox-db): move <BatchN> to vox-db-types`  (× 3-5 batches)
12. `refactor(<consumer>): depend on vox-db-types directly`  (× 2)
13. `feat(vox-db-types): derive Serialize/Deserialize on all row/entry types`
14. `feat(ci): row-serde-lint`
15. `feat(vox-db-types): add Db<Entity>Id string newtypes`
16. `refactor(vox-db-types): MemoryEntry uses DbAgentId/DbSessionId newtypes`
17. `feat(ci): string-id-lint (report-only)`
18. `feat(vox-db-types): add SessionRow → Session bridge + first caller migration`
19. `feat(vox-db-types): add <Pair2> bridge`  (× 2-3 more)
20. `docs(persistence): explain why operational JSON state is not in the DB`
21. `docs(adr-004): document sanctioned libSQL satellites + new CI checks`
22. `docs(architecture): clarify vox-db vs vox-db-types decision`
23. `docs(agents): document vox-db-types vs vox-db decision`
24. `chore(docs): regenerate auto-generated indices`

---

## Appendix C — When to STOP and ask the user

Pause and report rather than proceed if:

- A guard fails on baseline (Phase 0) for reasons not explained in the audit — the audit's premise may be wrong.
- A type move (Phase 4) requires inverting a layer relationship not yet in `[[known_inversions]]`.
- A row→domain bridge (Phase 6) requires inventing a new domain type (out of scope).
- The serde derive sweep (Phase 5) breaks a downstream wire format (private struct field becomes public via `Serialize`).
- `vox-arch-check` fails after a refactor and the fix is non-obvious.
- Total diff exceeds ~3000 lines — consider splitting Phase 4 + 6 + 7 into a follow-up PR.

The user's explicit preference (memory: `feedback_scope_check.md`) is to recommend before building. If something looks like a non-trivial detour, surface it.
