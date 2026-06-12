# Policy Registry — Phase 1b: All Remaining Domains Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the Phase 1a unified policy registry so the bootstrap generator + `policy-registry-parity` gate cover **all** remaining governable domains — `ci-gate`, `arch-rule`, `crl-gate`, `audit-check` — not just `code-audit-rule`. After 1b, `contracts/policy/policy-registry.v1.yaml` contains every governable policy, and parity fails on drift in **any** domain.

**Architecture:** Phase 1a wired `code-audit-rule` end-to-end with a generator + per-set/count/schema parity, all gated behind the `completion-toestub` feature (it links the optional `vox-code-audit` detector registry). Phase 1b adds four enumerators that read **plain YAML/TOML/in-crate registries** — none of which need `vox-code-audit`. They therefore compile in the **default** build. The generator becomes a domain-merge: code-audit (feature-gated) ⊕ ci-gate ⊕ arch ⊕ crl ⊕ audit (always available). `run_parity` runs per-set + count + schema checks **per domain**. The full-catalog regenerate that includes code-audit still requires `completion-toestub`; the four new domains' parity runs feature-independently.

**Tech Stack:** Rust, `serde` + `serde_yaml` + `toml`, the in-crate `vox_audit::registry()`, the existing clap `build_catalog()`. Tests via `cargo test -p <crate>`. Format with `cargo fmt -p <crate>` (never `--all` on Windows). Gates run with `--features completion-toestub` + `VOX_SKIP_FRESHNESS_CHECK=1` for debug binaries.

**Scope note:** Phase 1b of the initiative in
[`docs/superpowers/specs/2026-06-06-unified-policy-registry-and-governance-surface-design.md`](../specs/2026-06-06-unified-policy-registry-and-governance-surface-design.md).
Read its §4.2/§4.3 and the §10 verification addendum first — the addendum pins the
exact enumeration sources and overrides the body where they differ. Builds on
[`Plan 1a`](2026-06-06-policy-registry-phase-1a-catalog-foundation.md).

---

## Verified enumeration sources (hand-checked against real code)

| Domain | Source artifact | Live count | Struct / fn (file:line) |
|---|---|---|---|
| `ci-gate` | `contracts/operations/catalog.v1.yaml` (`ci.*` ops) | **78** `ci.*` ops | `OperationsCatalog` / `OperationRow` — `crates/vox-cli/src/commands/ci/operations_catalog.rs:56` / `:111` (fields `id`, `title`, `description`, `description_human`). Cross-check via `build_catalog()` → `crates/vox-cli/src/command_catalog.rs:77` (`CommandCatalogEntry.capability_id: Option<String>`, `:66`). |
| `arch-rule` | `docs/src/architecture/layers.toml` `[guards]` | **11** keys | Parsed directly with `toml` — `layers.toml:37` `[guards]`; keys `fan_in/loc_budget/orphan/docstring/description/where_things_live/wtl_parity/loc_delta/staleness/generated_file_drift/forbidden_deps` (`layers.toml:40-56`); value is `"warn"`/`"error"` (`strict` accepted). No public `GuardsConfig` struct re-exported from `vox-arch-check`, so parse the TOML in `vox-cli` (NOTE below). |
| `crl-gate` | `vox_audit::registry()` + `CrlGate` | **10** (9 CR-L + 1 Tooling) | `CrlGate` enum — `crates/vox-audit/src/lib.rs:34`; `pub fn registry()` → `crates/vox-audit/src/lib.rs:199`; `Subcommand::gate()`/`description()` → `:184`/`:187`; `CrlGate::thing_name()`/`block_ga()` → `:54`/`:73`. `vox-audit` is a **non-optional** dep of `vox-cli` (`Cargo.toml:261`). |
| `audit-check` | `contracts/ci/check-targets.v1.yaml` | **26** entries | `CheckManifest` / `CheckEntry` — `crates/vox-cli/src/commands/audit.rs:51` / `:58` (fields `id`, `description`, `category`, `blocking`, `runs_on`, `rust_only`, `command`, `quick_skip`). Both are `pub`; `commands::audit` is `pub mod` (`commands/mod.rs:15`). |

**Build-independence (verified `crates/vox-cli/Cargo.toml`):** `vox-config` (`:162`), `vox-audit` (`:261`), `vox-jsonschema-util` (`:221`), `vox-bounded-fs` (`:220`) are all **non-optional**. `toml` — confirm it is a `vox-cli` dep in Task 0; it is a near-universal workspace dep. Only `vox-code-audit` is optional (`:180`, feature `completion-toestub`, `:89`). **Therefore the four new domains compile in the default build; only `code_audit_entries()` stays feature-gated.**

**Infeasibility / deviations found:**
- **arch `GuardsConfig` is not a re-exported public struct.** `vox-arch-check` is a binary crate (`src/main.rs`) with a private `ArchCheckConfig` (`main.rs:117`); there is no library API to call. Plan 1b parses `layers.toml [guards]` directly with `toml` in `vox-cli` (the TOML shape is stable and the 11 keys are pinned by the §10 addendum). NOTED in Task 3.
- **ci-gate id-space mismatch.** Operation ids are dotted (`ci.artifact-audit`); the clap catalog uses `path: Vec<String>` (`["ci","artifact-audit"]`) and a `capability_id` like `cli.ci.artifact-audit`. The cross-check normalizes both to the dotted `ci.<leaf>` form before diffing. NOTED in Task 2.
- **crl `severity`.** CR-L gates have no severity enum; map `block_ga()==true → Error`, else `Warn`. NOTED in Task 4.

---

## File Structure

**Modify:**
- `crates/vox-cli/src/commands/ci/policy_registry.rs` — add four enumerators, fix `group_for`, merge into `build_registry`, extend `run_parity` to per-domain.
- `crates/vox-config/src/policy/registry.rs` — (only if needed) confirm `PolicyDomain::{CiGate,ArchRule,CrlGate,AuditCheck}` already exist (they do — `:44-51`). No model change expected.
- `contracts/policy/policy-registry.v1.yaml` — regenerated to include all domains.
- `crates/vox-cli/Cargo.toml` — add `toml = { workspace = true }` under `[dependencies]` only if Task 0 shows it absent.

**No new files.** The schema (`contracts/policy/policy-registry.v1.schema.json`) already permits all six domain enum values (Plan 1a Task 3); no schema change.

---

## Task 0: Pre-flight — confirm deps + current green baseline

**Files:** none (verification only)

- [ ] **Step 1: Confirm the non-code-audit deps are non-optional and `toml` is available**

```bash
grep -nE 'vox-audit|vox-config|vox-jsonschema-util|vox-bounded-fs|^toml ' crates/vox-cli/Cargo.toml
```
Expected: `vox-audit`, `vox-config`, `vox-jsonschema-util`, `vox-bounded-fs` present **without** `optional = true`. If `toml` is **absent**, add this line under `[dependencies]` and commit separately:

```toml
toml = { workspace = true }
```

- [ ] **Step 2: Confirm the live counts match this plan's assumptions**

```bash
grep -c '^- id: ci\.' contracts/operations/catalog.v1.yaml          # expect 78
grep -c '^  - id:' contracts/ci/check-targets.v1.yaml               # expect 26
grep -cE '^(fan_in|loc_budget|orphan|docstring|description|where_things_live|wtl_parity|loc_delta|staleness|generated_file_drift|forbidden_deps)' docs/src/architecture/layers.toml  # expect 11
```
> If any count differs, the parity gate will still be correct (it derives counts
> from live sources); only the `>=` lower bounds in the tests below need adjusting
> to `live - small_margin`. Note the real numbers in the test comments.

- [ ] **Step 3: Confirm Phase 1a is green (the thing we extend)**

```bash
VOX_SKIP_FRESHNESS_CHECK=1 cargo test -p vox-cli --features completion-toestub commands::ci::policy_registry::tests
```
Expected: PASS (4 tests from Plan 1a).

---

## Task 1: `audit-check` enumerator (simplest — plain YAML, default build)

**Files:** Modify `crates/vox-cli/src/commands/ci/policy_registry.rs`

- [ ] **Step 1: Write the failing test**

Add to (or create) a **feature-independent** tests module in `policy_registry.rs`. Phase 1a's
tests module is `#[cfg(all(test, feature = "completion-toestub"))]`; add a **separate**
module that does NOT require the feature so the new domains are tested in the default build:

```rust
#[cfg(test)]
mod default_domain_tests {
    use super::*;
    use std::path::Path;

    fn repo_root() -> std::path::PathBuf {
        // policy_registry.rs is at crates/vox-cli/src/commands/ci/, so 4 ancestors up
        // (ci → commands → src → vox-cli → crates) then one more to repo root.
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(1)
            .expect("repo root")
            .to_path_buf()
    }

    #[test]
    fn audit_check_entries_cover_check_targets() {
        let entries = audit_check_entries(&repo_root()).expect("load check-targets");
        // Live check-targets.v1.yaml has 26 entries (verified 2026-06-06).
        assert!(
            entries.len() >= 24,
            "expected ~26 check-targets, got {}",
            entries.len()
        );
        let fmt = entries
            .iter()
            .find(|e| e.id == "audit-check/fmt")
            .expect("fmt check should be present");
        assert_eq!(fmt.domain, vox_config::PolicyDomain::AuditCheck);
        assert_eq!(fmt.source.kind, vox_config::PolicySourceKind::Command);
        assert!(fmt.blocking, "fmt is blocking: true in the manifest");
        assert!(fmt.group.starts_with("Audit checks"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vox-cli default_domain_tests::audit_check_entries_cover_check_targets
```
Expected: FAIL — `cannot find function audit_check_entries`.

- [ ] **Step 3: Write the enumerator**

Add to `policy_registry.rs` (top-level, **not** feature-gated). Import the model types at
crate level (Phase 1a only imported them under the feature); add an unconditional import:

```rust
// Unconditional model imports for the default-build domains (ci-gate / arch /
// crl / audit). Phase 1a's feature-gated `use` above remains for code-audit.
use vox_config::{PolicyDomain, PolicyEntry, PolicySource, PolicySourceKind, PolicySeverity};
```

> If this produces a duplicate-import / unused-import warning against the Phase 1a
> `#[cfg(feature = "completion-toestub")] use vox_config::{...}` line, delete the
> feature-gated `use` and rely on this unconditional one (the code-audit functions
> are themselves feature-gated, so the import is "used" only when the feature is on,
> which is fine — `use` of items needed by cfg'd code does not warn). Verify with
> `cargo build -p vox-cli` and `cargo build -p vox-cli --features completion-toestub`.

```rust
/// Enumerate `contracts/ci/check-targets.v1.yaml` into `audit-check` entries.
pub fn audit_check_entries(repo_root: &Path) -> Result<Vec<PolicyEntry>, String> {
    use crate::commands::audit::CheckManifest;
    let path = repo_root.join("contracts/ci/check-targets.v1.yaml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let manifest: CheckManifest =
        serde_yaml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let mut out: Vec<PolicyEntry> = manifest
        .checks
        .iter()
        .map(|c| {
            let blocking = c.blocking;
            PolicyEntry {
                id: format!("audit-check/{}", c.id),
                domain: PolicyDomain::AuditCheck,
                title: c.id.clone(),
                group: format!("Audit checks / {}", c.category),
                description: c.description.clone(),
                severity: Some(if blocking {
                    PolicySeverity::Error
                } else {
                    PolicySeverity::Warn
                }),
                blocking,
                runs_on: c.runs_on.clone(),
                source: PolicySource {
                    kind: PolicySourceKind::Command,
                    reference: format!("contracts/ci/check-targets.v1.yaml#{}", c.id),
                    detail: Some(c.command.join(" ")),
                },
                docs: None,
                default_enabled: true,
                protected: false,
                origin: "builtin".to_string(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p vox-cli default_domain_tests::audit_check_entries_cover_check_targets
```
Expected: PASS.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/policy_registry.rs
git commit -m "feat(vox-cli): policy registry audit-check enumerator (check-targets)"
```

---

## Task 2: `ci-gate` enumerator + clap cross-check

**Files:** Modify `crates/vox-cli/src/commands/ci/policy_registry.rs`

- [ ] **Step 1: Write the failing test**

Add to `default_domain_tests`:

```rust
#[test]
fn ci_gate_entries_cover_operations_catalog() {
    let entries = ci_gate_entries(&repo_root()).expect("load operations catalog");
    // catalog.v1.yaml has 78 `ci.*` operations (verified 2026-06-06).
    assert!(entries.len() >= 70, "expected ~78 ci ops, got {}", entries.len());
    let parity = entries
        .iter()
        .find(|e| e.id == "ci-gate/ci.capability-sync")
        .expect("ci.capability-sync op present");
    assert_eq!(parity.domain, vox_config::PolicyDomain::CiGate);
    assert_eq!(parity.source.kind, vox_config::PolicySourceKind::Command);
    assert!(parity.group.starts_with("CI Gates"));
}

#[test]
fn ci_gate_clap_crosscheck_has_no_orphans() {
    // Every `ci.*` op in the catalog should correspond to a live clap path
    // `vox ci <leaf>`; a catalog op with no clap command is enum↔catalog drift.
    let orphans = ci_gate_catalog_clap_drift(&repo_root());
    assert!(
        orphans.is_empty(),
        "ci ops with no live clap subcommand (enum↔catalog drift): {orphans:?}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vox-cli default_domain_tests::ci_gate
```
Expected: FAIL — `cannot find function ci_gate_entries` / `ci_gate_catalog_clap_drift`.

- [ ] **Step 3: Write the enumerator + cross-check**

```rust
/// Enumerate the `ci.*` operations from `contracts/operations/catalog.v1.yaml`
/// into `ci-gate` entries. Uses the existing `OperationsCatalog`/`OperationRow`
/// serde structs (operations_catalog.rs) so this stays a single SSOT.
pub fn ci_gate_entries(repo_root: &Path) -> Result<Vec<PolicyEntry>, String> {
    use crate::commands::ci::operations_catalog::OperationsCatalog;
    let path = repo_root.join("contracts/operations/catalog.v1.yaml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let catalog: OperationsCatalog =
        serde_yaml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let mut out: Vec<PolicyEntry> = catalog
        .operations
        .iter()
        .filter(|op| op.id.starts_with("ci."))
        .map(|op| {
            // `ci.check-summary-drift` → group "CI Gates / check" (first id segment after `ci.`).
            let leaf = op.id.strip_prefix("ci.").unwrap_or(&op.id);
            let head = leaf.split('-').next().unwrap_or(leaf);
            let desc = op
                .description_human
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| op.description.clone());
            PolicyEntry {
                id: format!("ci-gate/{}", op.id),
                domain: PolicyDomain::CiGate,
                title: op.title.clone(),
                group: format!("CI Gates / {head}"),
                description: desc,
                // Catalog rows carry no severity; CI gates are blocking by convention.
                severity: Some(PolicySeverity::Error),
                blocking: true,
                runs_on: vec!["ci".into()],
                source: PolicySource {
                    kind: PolicySourceKind::Command,
                    reference: format!("contracts/operations/catalog.v1.yaml#{}", op.id),
                    detail: Some(format!("vox ci {}", leaf)),
                },
                docs: None,
                default_enabled: true,
                protected: false,
                origin: "builtin".to_string(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// Cross-check: return any `ci.<leaf>` op id whose `vox ci <leaf>` command is NOT
/// present in the live clap catalog (enum↔catalog drift). Normalizes both sides to
/// the dotted `ci.<leaf>` form before diffing. Clap is the live source of truth for
/// "this subcommand actually exists".
pub fn ci_gate_catalog_clap_drift(repo_root: &Path) -> Vec<String> {
    use crate::command_catalog::build_catalog;
    use std::collections::BTreeSet;

    // Live `vox ci <leaf>` paths from clap, normalized to `ci.<leaf>`.
    let live: BTreeSet<String> = build_catalog()
        .entries
        .iter()
        .filter(|e| e.path.first().map(|s| s == "ci").unwrap_or(false) && e.path.len() == 2)
        .map(|e| format!("ci.{}", e.path[1]))
        .collect();

    let Ok(entries) = ci_gate_entries(repo_root) else {
        return vec!["<failed to load operations catalog>".to_string()];
    };
    entries
        .iter()
        .map(|e| e.id.trim_start_matches("ci-gate/").to_string())
        .filter(|op_id| !live.contains(op_id))
        .collect()
}
```

> **NOTE (id-space mismatch):** operation ids are dotted (`ci.capability-sync`)
> while clap exposes `path: ["ci", "capability-sync"]`. The cross-check normalizes
> the clap path to `ci.<leaf>`. If the live catalog deliberately lists CI ops with no
> standalone `vox ci <leaf>` (e.g. flag-only ops), the cross-check will flag them;
> in that case, downgrade `ci_gate_catalog_clap_drift` to a **warning** printed in
> `run_parity` rather than a hard error, and record the known exceptions in a small
> `const CI_CLAP_DRIFT_ALLOW: &[&str]` with a comment. Decide based on the actual
> Step-2 failure list — do not pre-populate the allowlist speculatively.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p vox-cli default_domain_tests::ci_gate
```
Expected: PASS. If `ci_gate_clap_crosscheck_has_no_orphans` fails, apply the NOTE above.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/policy_registry.rs
git commit -m "feat(vox-cli): policy registry ci-gate enumerator + clap cross-check"
```

---

## Task 3: `arch-rule` enumerator (layers.toml `[guards]`)

**Files:** Modify `crates/vox-cli/src/commands/ci/policy_registry.rs`

> **NOTE (no public GuardsConfig):** `vox-arch-check` is a **binary** crate with a
> private `ArchCheckConfig` (`crates/vox-arch-check/src/main.rs:117`); there is no
> library API to enumerate the guards. We therefore parse `layers.toml [guards]`
> directly with the `toml` crate. The 11 keys are pinned by the spec §10 addendum,
> so the parser is robust against new keys (any extra key becomes an extra entry,
> which the parity gate would surface — that is the desired drift behavior).

- [ ] **Step 1: Write the failing test**

Add to `default_domain_tests`:

```rust
#[test]
fn arch_rule_entries_cover_guards() {
    let entries = arch_rule_entries(&repo_root()).expect("parse layers.toml [guards]");
    // layers.toml [guards] has exactly 11 keys (verified 2026-06-06).
    assert_eq!(entries.len(), 11, "expected 11 arch guards, got {}", entries.len());
    let orphan = entries
        .iter()
        .find(|e| e.id == "arch-rule/orphan")
        .expect("orphan guard present");
    assert_eq!(orphan.domain, vox_config::PolicyDomain::ArchRule);
    assert_eq!(orphan.severity, Some(vox_config::PolicySeverity::Error)); // orphan = "error"
    assert!(orphan.blocking);
    let fan_in = entries.iter().find(|e| e.id == "arch-rule/fan_in").unwrap();
    assert_eq!(fan_in.severity, Some(vox_config::PolicySeverity::Warn)); // fan_in = "warn"
    assert!(!fan_in.blocking);
    assert_eq!(orphan.source.kind, vox_config::PolicySourceKind::Guard);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vox-cli default_domain_tests::arch_rule_entries_cover_guards
```
Expected: FAIL — `cannot find function arch_rule_entries`.

- [ ] **Step 3: Write the enumerator**

```rust
/// Parse `docs/src/architecture/layers.toml` `[guards]` into `arch-rule` entries.
/// Each guard key is one entry; severity comes from its `warn`/`error`/`strict`
/// value (`strict` maps to Critical, `error` → Error, anything else → Warn).
pub fn arch_rule_entries(repo_root: &Path) -> Result<Vec<PolicyEntry>, String> {
    let path = repo_root.join("docs/src/architecture/layers.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let doc: toml::Value =
        toml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let guards = doc
        .get("guards")
        .and_then(|v| v.as_table())
        .ok_or_else(|| format!("{} has no [guards] table", path.display()))?;

    let human = |key: &str| -> &'static str {
        match key {
            "fan_in" => "Crate fan-in budget",
            "loc_budget" => "Per-crate lines-of-code budget",
            "orphan" => "Orphan-library detection",
            "docstring" => "Crate-level docstring presence",
            "description" => "Cargo.toml description presence",
            "where_things_live" => "where-things-live coverage",
            "wtl_parity" => "WTL / layers.toml / disk three-way parity",
            "loc_delta" => "LoC delta regression vs last release tag",
            "staleness" => "Crate staleness vs last CHANGELOG release",
            "generated_file_drift" => "@generated hash header drift",
            "forbidden_deps" => "Forbidden direct dependencies",
            _ => "Architecture guard",
        }
    };

    let mut out: Vec<PolicyEntry> = guards
        .iter()
        .map(|(key, val)| {
            let level = val.as_str().unwrap_or("warn");
            let (severity, blocking) = match level {
                "strict" => (PolicySeverity::Critical, true),
                "error" => (PolicySeverity::Error, true),
                _ => (PolicySeverity::Warn, false),
            };
            PolicyEntry {
                id: format!("arch-rule/{key}"),
                domain: PolicyDomain::ArchRule,
                title: human(key).to_string(),
                group: "Architecture".to_string(),
                description: format!(
                    "{} (layers.toml [guards].{key} = \"{level}\")",
                    human(key)
                ),
                severity: Some(severity),
                blocking,
                runs_on: vec!["pre-push".into(), "ci".into()],
                source: PolicySource {
                    kind: PolicySourceKind::Guard,
                    reference: format!("docs/src/architecture/layers.toml#guards.{key}"),
                    detail: Some(format!("vox-arch-check guard `{key}`")),
                },
                docs: None,
                default_enabled: true,
                // Layer order + orphan + forbidden-deps are structural; protect them.
                protected: matches!(key.as_str(), "orphan" | "forbidden_deps" | "where_things_live"),
                origin: "builtin".to_string(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p vox-cli default_domain_tests::arch_rule_entries_cover_guards
```
Expected: PASS.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/policy_registry.rs
git commit -m "feat(vox-cli): policy registry arch-rule enumerator (layers.toml guards)"
```

---

## Task 4: `crl-gate` enumerator (`vox_audit::registry()`)

**Files:** Modify `crates/vox-cli/src/commands/ci/policy_registry.rs`

- [ ] **Step 1: Write the failing test**

Add to `default_domain_tests`:

```rust
#[test]
fn crl_gate_entries_cover_audit_registry() {
    let entries = crl_gate_entries();
    // vox_audit::registry() = 9 CR-L gates + 1 tooling gate (verified 2026-06-06).
    assert_eq!(entries.len(), 10, "expected 10 CR-L/tooling gates, got {}", entries.len());
    let l0 = entries
        .iter()
        .find(|e| e.id == "crl-gate/spec-to-app")
        .expect("spec-to-app (CR-L0) present");
    assert_eq!(l0.domain, vox_config::PolicyDomain::CrlGate);
    assert!(l0.blocking, "CR-L0 block_ga() == true");
    assert_eq!(l0.severity, Some(vox_config::PolicySeverity::Error));
    let tooling = entries
        .iter()
        .find(|e| e.id == "crl-gate/stdlib-coverage")
        .expect("tooling gate present");
    assert!(!tooling.blocking, "tooling gate block_ga() == false");
    assert_eq!(tooling.severity, Some(vox_config::PolicySeverity::Warn));
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vox-cli default_domain_tests::crl_gate_entries_cover_audit_registry
```
Expected: FAIL — `cannot find function crl_gate_entries`.

- [ ] **Step 3: Write the enumerator**

```rust
/// Enumerate the in-crate `vox_audit::registry()` (CrlGate + Subcommand metadata)
/// into `crl-gate` entries. `vox-audit` is a non-optional dep of `vox-cli`, so this
/// compiles in the default build. CR-L gates carry no severity; `block_ga()` maps
/// to Error (blocks GA) vs Warn.
pub fn crl_gate_entries() -> Vec<PolicyEntry> {
    let mut out: Vec<PolicyEntry> = vox_audit::registry()
        .iter()
        .map(|sub| {
            let gate = sub.gate();
            let thing = gate.thing_name();
            let blocks = gate.block_ga();
            PolicyEntry {
                id: format!("crl-gate/{thing}"),
                domain: PolicyDomain::CrlGate,
                title: format!("CR-L gate: {thing}"),
                group: "CR-L gates".to_string(),
                description: sub.description().to_string(),
                severity: Some(if blocks {
                    PolicySeverity::Error
                } else {
                    PolicySeverity::Warn
                }),
                blocking: blocks,
                runs_on: vec!["ci".into()],
                source: PolicySource {
                    kind: PolicySourceKind::Command,
                    reference: format!("contracts/ci/vox-audit-contract.v1.yaml#{thing}"),
                    detail: Some(format!("vox audit {thing}")),
                },
                docs: None,
                default_enabled: true,
                // GA-blocking CR-L gates are policy-protected.
                protected: blocks,
                origin: "builtin".to_string(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p vox-cli default_domain_tests::crl_gate_entries_cover_audit_registry
```
Expected: PASS.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/policy_registry.rs
git commit -m "feat(vox-cli): policy registry crl-gate enumerator (vox-audit registry)"
```

---

## Task 5: Fix `group_for` + merge all domains into `build_registry`

**Files:** Modify `crates/vox-cli/src/commands/ci/policy_registry.rs`

The Phase 1a `group_for` matched **wrong** id prefixes (`stub`/`victory-claim`/`secrets`/
`llm_provider_call`). The §10 addendum + the landed code (`protected: raw_id == "arch/stub"
|| raw_id == "vox/llm/direct-provider-call"`) confirm real ids are namespaced
`arch/`, `vox/`, `security/`, `skeleton/`, `scaling/`, `ai-laziness/`, `victory-claim/`.

- [ ] **Step 1: Write the failing test (feature-gated — touches code-audit ids)**

Add to the Phase 1a `#[cfg(all(test, feature = "completion-toestub"))] mod tests`:

```rust
#[test]
fn group_for_uses_real_namespaced_prefixes() {
    // Real detector ids are namespaced (arch/, vox/, security/, …) — verified
    // against vox_code_audit::detectors::all_rules ids in the landed `protected`
    // mapping. The Phase 1a labels (stub/secrets/…) never matched live ids.
    assert_eq!(group_for("code-audit/arch/stub"), "Language rules / Architecture");
    assert_eq!(
        group_for("code-audit/vox/llm/direct-provider-call"),
        "Language rules / Vox idioms"
    );
    assert_eq!(group_for("code-audit/security/hardcoded-secret"), "Language rules / Security");
    assert_eq!(group_for("code-audit/scaling/n-plus-one"), "Language rules / Scaling");
    assert_eq!(group_for("code-audit/victory-claim/premature-done"), "Language rules / Victory claims");
    assert_eq!(group_for("code-audit/ai-laziness/silent-catch"), "Language rules / AI-laziness");
    assert_eq!(group_for("code-audit/skeleton/empty-fn"), "Language rules / Skeletons");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vox-cli --features completion-toestub commands::ci::policy_registry::tests::group_for_uses_real_namespaced_prefixes
```
Expected: FAIL — current `group_for` returns `"Language rules / General"` for these.

- [ ] **Step 3: Fix `group_for`**

Replace the Phase 1a `group_for` body with the real prefix mapping:

```rust
/// Derive a human group label from a `code-audit/<namespace>/...` id.
/// Real detector ids are namespaced (verified against
/// `vox_code_audit::detectors::all_rules`): arch/, vox/, security/, skeleton/,
/// scaling/, ai-laziness/, victory-claim/.
#[cfg(feature = "completion-toestub")]
fn group_for(id: &str) -> String {
    let body = id.strip_prefix("code-audit/").unwrap_or(id);
    let head = body.split('/').next().unwrap_or(body);
    let label = match head {
        "arch" => "Architecture",
        "vox" => "Vox idioms",
        "security" => "Security",
        "skeleton" => "Skeletons",
        "scaling" => "Scaling",
        "ai-laziness" => "AI-laziness",
        "victory-claim" => "Victory claims",
        _ => "General",
    };
    format!("Language rules / {label}")
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p vox-cli --features completion-toestub commands::ci::policy_registry::tests::group_for_uses_real_namespaced_prefixes
```
Expected: PASS.

- [ ] **Step 5: Merge all domains into `build_registry`**

The Phase 1a `build_registry()` is feature-gated and returns only code-audit. Replace it
with a feature-aware merge so the four new domains are always included and code-audit is
added when the feature is on. `build_registry` now needs `repo_root` (the YAML/TOML
domains read from disk):

```rust
/// Build the full registry document across all domains this plan covers.
/// The four contract/registry-backed domains (ci-gate, arch, crl, audit) compile
/// in the default build; `code-audit-rule` is added only under `completion-toestub`.
pub fn build_registry(repo_root: &Path) -> Result<vox_config::PolicyRegistry, String> {
    let mut policies: Vec<PolicyEntry> = Vec::new();
    policies.extend(ci_gate_entries(repo_root)?);
    policies.extend(arch_rule_entries(repo_root)?);
    policies.extend(crl_gate_entries());
    policies.extend(audit_check_entries(repo_root)?);

    #[cfg(feature = "completion-toestub")]
    policies.extend(code_audit_entries());

    policies.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(vox_config::PolicyRegistry {
        schema_version: 1,
        policies,
    })
}
```

> **Call-site updates:** `run_generate` (Phase 1a) calls `build_registry()` with no args
> and the feature gate. Update its feature-gated body to `build_registry(repo_root)?`
> and remove its `#[cfg(feature = "completion-toestub")]` gate (it now works in the
> default build — generation no longer *requires* code-audit; it just *includes* it when
> present). **Delete the `#[cfg(not(feature = "completion-toestub"))] fn run_generate`
> stub** — generation is no longer feature-dependent. Keep a one-line note in the
> non-feature path of the YAML header (see Step 6 of Task 6) that code-audit rows are
> omitted unless the feature is on. Adjust `run_generate` signature to propagate the
> `Result<_, String>` from `build_registry`.

- [ ] **Step 6: Build both configurations**

```bash
cargo build -p vox-cli
cargo build -p vox-cli --features completion-toestub
```
Expected: both succeed, no warnings.

- [ ] **Step 7: Format and commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/policy_registry.rs
git commit -m "fix(vox-cli): correct code-audit group_for prefixes; merge all domains into registry"
```

---

## Task 6: Per-domain parity in `run_parity` + regenerate catalog

**Files:** Modify `crates/vox-cli/src/commands/ci/policy_registry.rs`, `contracts/policy/policy-registry.v1.yaml`

- [ ] **Step 1: Write the failing test**

The Phase 1a parity is feature-gated and code-audit-only. Phase 1b makes the bulk of
parity feature-independent. Add a default-build test in `default_domain_tests`:

```rust
#[test]
fn parity_passes_for_default_domains_against_committed_yaml() {
    // After regenerate (Step 4), the committed YAML must contain every live
    // ci-gate / arch / crl / audit item and no extras. This runs WITHOUT the
    // completion-toestub feature, proving the non-code-audit domains are gated
    // feature-independently (spec §4.4 build-independence requirement).
    let root = repo_root();
    run_parity_default_domains(&root).expect("default-domain parity must pass");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vox-cli default_domain_tests::parity_passes_for_default_domains_against_committed_yaml
```
Expected: FAIL — `cannot find function run_parity_default_domains` (and, once it exists,
it fails because the committed YAML has not yet been regenerated with the new domains).

- [ ] **Step 3: Write the per-domain parity engine**

Add a reusable per-domain checker and a default-domains entry point (both feature-independent):

```rust
use std::collections::BTreeSet;

/// One domain's expected (live) entries plus its `PolicyDomain` tag.
struct DomainExpectation {
    domain: PolicyDomain,
    label: &'static str,
    expected: Vec<PolicyEntry>,
}

/// Assert, for each domain, that the on-disk registry rows of that domain exactly
/// match the live source set: same count (no dup clones), every live id present,
/// no stale id. Preserves the Phase 1a count + set semantics, applied per domain.
fn check_domain_parity(
    on_disk: &vox_config::PolicyRegistry,
    exp: &DomainExpectation,
) -> Result<usize, String> {
    let disk_rows: Vec<&str> = on_disk
        .policies
        .iter()
        .filter(|e| e.domain == exp.domain)
        .map(|e| e.id.as_str())
        .collect();
    let disk_ids: BTreeSet<&str> = disk_rows.iter().copied().collect();
    let exp_ids: BTreeSet<&str> = exp.expected.iter().map(|e| e.id.as_str()).collect();

    if disk_rows.len() != exp_ids.len() {
        return Err(format!(
            "policy registry duplicate/count mismatch ({} domain): on-disk has {} row(s) but {} unique live id(s); run `vox ci policy-registry --write`",
            exp.label,
            disk_rows.len(),
            exp_ids.len()
        ));
    }
    let missing: Vec<&str> = exp_ids.difference(&disk_ids).copied().collect();
    let extra: Vec<&str> = disk_ids.difference(&exp_ids).copied().collect();
    if !missing.is_empty() || !extra.is_empty() {
        return Err(format!(
            "policy registry drift ({} domain):\n  missing {}: {:?}\n  stale {}: {:?}\n  run `vox ci policy-registry --write`",
            exp.label,
            missing.len(),
            missing,
            extra.len(),
            extra
        ));
    }
    Ok(exp_ids.len())
}

/// Build expectations for the four contract/registry-backed domains.
fn default_domain_expectations(repo_root: &Path) -> Result<Vec<DomainExpectation>, String> {
    Ok(vec![
        DomainExpectation {
            domain: PolicyDomain::CiGate,
            label: "ci-gate",
            expected: ci_gate_entries(repo_root)?,
        },
        DomainExpectation {
            domain: PolicyDomain::ArchRule,
            label: "arch-rule",
            expected: arch_rule_entries(repo_root)?,
        },
        DomainExpectation {
            domain: PolicyDomain::CrlGate,
            label: "crl-gate",
            expected: crl_gate_entries(),
        },
        DomainExpectation {
            domain: PolicyDomain::AuditCheck,
            label: "audit-check",
            expected: audit_check_entries(repo_root)?,
        },
    ])
}

/// Feature-independent parity over the four default-build domains, including the
/// clap↔catalog cross-check and JSON-Schema validation of the committed YAML.
pub fn run_parity_default_domains(repo_root: &Path) -> Result<(), String> {
    let on_disk = vox_config::load_policy_registry(repo_root).map_err(|e| e.to_string())?;
    validate_registry_against_schema(repo_root)?;

    // enum↔catalog cross-check for ci-gate (see Task 2 NOTE for allowlist option).
    let orphans = ci_gate_catalog_clap_drift(repo_root);
    if !orphans.is_empty() {
        return Err(format!(
            "ci-gate enum↔catalog drift: {} op(s) have no live `vox ci <leaf>` command: {orphans:?}",
            orphans.len()
        ));
    }

    let mut total = 0usize;
    for exp in default_domain_expectations(repo_root)? {
        total += check_domain_parity(&on_disk, &exp)?;
    }
    println!("policy-registry-parity OK (default domains): {total} ci-gate+arch+crl+audit entries match live sources");
    Ok(())
}
```

> **`validate_registry_against_schema` is feature-gated in Phase 1a** (it lives under
> `#[cfg(feature = "completion-toestub")]`). Move that fn (and its
> `vox_jsonschema_util` use) **out** of the feature gate so it is available in the
> default build — `vox-jsonschema-util` is a non-optional dep (`Cargo.toml:221`), so
> this compiles unconditionally. Do the same for the `validate_registry_against_schema`
> call inside the feature-gated `run_parity`.

- [ ] **Step 4: Make the feature-gated `run_parity` call the per-domain engine for ALL domains**

Update the Phase 1a `#[cfg(feature = "completion-toestub")] run_parity` so it checks
**every** domain (code-audit + the four defaults) via the shared `check_domain_parity`,
preserving its schema-validation + count + set assertions:

```rust
/// `vox ci policy-registry-parity`: assert the committed registry matches all live
/// sources exactly (no drift), per domain. Includes code-audit when the feature is on.
#[cfg(feature = "completion-toestub")]
pub fn run_parity(repo_root: &Path) -> Result<(), String> {
    let on_disk = vox_config::load_policy_registry(repo_root).map_err(|e| e.to_string())?;
    validate_registry_against_schema(repo_root)?;

    let orphans = ci_gate_catalog_clap_drift(repo_root);
    if !orphans.is_empty() {
        return Err(format!(
            "ci-gate enum↔catalog drift: {} op(s) have no live `vox ci <leaf>` command: {orphans:?}",
            orphans.len()
        ));
    }

    let mut expectations = default_domain_expectations(repo_root)?;
    expectations.push(DomainExpectation {
        domain: PolicyDomain::CodeAuditRule,
        label: "code-audit",
        expected: code_audit_entries(),
    });

    let mut total = 0usize;
    for exp in &expectations {
        total += check_domain_parity(&on_disk, exp)?;
    }
    println!("policy-registry-parity OK: {total} entries across {} domains match live sources", expectations.len());
    Ok(())
}
```

> **Without the feature**, replace the Phase 1a `#[cfg(not(feature = "completion-toestub"))]
> run_parity` stub (which errored) with one that runs the four default domains and prints
> a clear note that code-audit parity is skipped:
>
> ```rust
> #[cfg(not(feature = "completion-toestub"))]
> pub fn run_parity(repo_root: &Path) -> Result<(), String> {
>     run_parity_default_domains(repo_root)?;
>     println!("(note: code-audit-rule parity skipped — rebuild with `--features completion-toestub` to enforce it)");
>     Ok(())
> }
> ```
>
> This realizes the spec §4.4 requirement: non-code-audit parity runs feature-independently.

- [ ] **Step 5: Regenerate the catalog (full, with code-audit)**

```bash
VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli --features completion-toestub -- ci policy-registry --write
```
Expected: writes `contracts/policy/policy-registry.v1.yaml` with all domains; prints
`wrote ... (N policies)` where N ≈ 78 + 11 + 10 + 26 + 51 = **~176**.

- [ ] **Step 6: Verify parity both ways**

```bash
VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli --features completion-toestub -- ci policy-registry-parity
VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli -- ci policy-registry-parity
```
Expected: feature build prints `OK: ~176 entries across 5 domains`; default build prints
`OK (default domains): ~125 ...` + the code-audit-skipped note. Then:

```bash
cargo test -p vox-cli default_domain_tests
cargo test -p vox-cli --features completion-toestub commands::ci::policy_registry::tests
```
Expected: all PASS.

- [ ] **Step 7: Format and commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/policy_registry.rs contracts/policy/policy-registry.v1.yaml
git commit -m "feat(vox-cli): per-domain policy-registry parity; regenerate full catalog"
```

---

## Task 7: Verify `vox policy list/domains/groups` show all domains

**Files:** none (the Phase 1a `vox policy` CLI already reads the catalog generically)

- [ ] **Step 1: Confirm the CLI surfaces every new domain**

```bash
VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli -- policy domains
VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli -- policy list --domain ci-gate
VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli -- policy list --domain arch-rule
VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli -- policy groups
VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli -- policy show arch-rule/orphan
VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli -- policy show crl-gate/spec-to-app
```
Expected: `domains` lists `arch-rule`, `audit-check`, `ci-gate`, `code-audit-rule`,
`crl-gate`; `list --domain ci-gate` shows ~78 rows; `groups` shows `Architecture`,
`Audit checks / *`, `CI Gates / *`, `CR-L gates`, `Language rules / *`; `show` prints the
guard/gate rule contents (kind/source/detail).

> The Phase 1a `matches_filter` filters domain via `serde_domain(e)` (the kebab-case
> serialized form), so `--domain ci-gate` works without code changes. If `--domain`
> matching is unexpectedly empty, confirm `serde_domain` yields `ci-gate` (it should —
> `PolicyDomain` is `#[serde(rename_all = "kebab-case")]`).

- [ ] **Step 2: Add the CI gate to the audit/check-targets registry (self-registration)**

The spec §4.3 says `policy-registry-parity` is itself a `ci-gate`. Confirm it appears in
the regenerated catalog as `ci-gate/ci.policy-registry-parity` **iff** a corresponding
`ci.policy-registry-parity` op exists in `contracts/operations/catalog.v1.yaml`.

```bash
grep -n 'policy-registry' contracts/operations/catalog.v1.yaml
```
> If the op is **absent**, that is expected (Plan 1a added the `CiCmd` variants but not
> a catalog op). Adding the catalog op is governed by the existing
> `vox ci capability-sync` / operations-catalog flow and is **out of scope** for 1b —
> NOTE it in the Self-Review "what this defers" and do not hand-edit the catalog here
> (it would fail the operations-catalog parity gate). The policy-registry gate will pick
> the op up automatically once it is added through the proper channel.

- [ ] **Step 3: Commit (only if anything changed)**

No file changes expected in this task. If Step 1 surfaced a `serde_domain` fix, commit it:

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/policy/mod.rs
git commit -m "fix(vox-cli): vox policy domain filter for new policy domains"
```

---

## Task 8: Final gates

**Files:** none (verification + where-things-live note check)

- [ ] **Step 1: Run architecture + workspace checks**

```bash
cargo run -p vox-arch-check
VOX_SKIP_FRESHNESS_CHECK=1 cargo test -p vox-cli policy
VOX_SKIP_FRESHNESS_CHECK=1 cargo test -p vox-cli --features completion-toestub commands::ci::policy_registry
```
Expected: arch-check green (the Plan 1a where-things-live row already covers the policy
catalog — no new crate added, so no new WTL row needed); all policy tests pass.

- [ ] **Step 2: Run the parity gate exactly as CI would**

```bash
VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli --features completion-toestub -- ci policy-registry-parity
```
Expected: `policy-registry-parity OK: ~176 entries across 5 domains match live sources`.

- [ ] **Step 3: Drift smoke test (prove the gate bites for a new domain)**

Temporarily delete one `ci-gate/*` row from `contracts/policy/policy-registry.v1.yaml`,
re-run parity, confirm it FAILS with a `ci-gate domain` drift message, then restore:

```bash
# (edit YAML: remove one ci-gate row)
VOX_SKIP_FRESHNESS_CHECK=1 cargo run -p vox-cli -- ci policy-registry-parity   # must FAIL, ci-gate domain
git checkout contracts/policy/policy-registry.v1.yaml
```
Expected: FAIL on the tampered file (proving per-domain parity is live in the **default**
build too), then clean after restore. No commit (working tree restored).

---

## Self-Review

**Spec coverage (Phase 1b slice):**
- `ci-gate` from `operations/catalog.v1.yaml` (78 `ci.*` ops) + clap cross-check → Task 2. ✓ (spec §10 correction 1)
- `arch-rule` from `layers.toml [guards]` (11 keys, severity from warn/error/strict) → Task 3. ✓
- `crl-gate` from `vox_audit::registry()` (9 CR-L + 1 tooling) → Task 4. ✓
- `audit-check` from `check-targets.v1.yaml` (26 entries) → Task 1. ✓
- `group_for` corrected to real namespaced prefixes (arch/vox/security/skeleton/scaling/ai-laziness/victory-claim) → Task 5. ✓
- Per-domain parity (count + set + schema) preserving Phase 1a semantics → Task 6. ✓
- Feature-independence: four new domains compile + parity in the default build; only code-audit gated by `completion-toestub` → Tasks 1–6 (`run_parity_default_domains`, default-build `run_parity`). ✓ (spec §4.4)
- Regenerated catalog + `vox policy` surfaces all domains → Tasks 6, 7. ✓

**Placeholder scan:** No "TBD"/bare references. The `>` notes are concrete verification
reminders with decision criteria (clap-drift allowlist only if Step 2 fails; toml dep
added only if absent; `validate_registry_against_schema` de-gating is a precise mechanical
move). No hollow functions — every enumerator returns real entries from real sources.

**Type consistency:** All four enumerators return `Vec<PolicyEntry>` (audit/ci/arch as
`Result<Vec<_>, String>` because they read disk; crl is infallible). `build_registry` now
takes `repo_root` and returns `Result`; both `run_generate` and both `run_parity` arms are
updated to match. `check_domain_parity` + `DomainExpectation` are shared by the feature and
default `run_parity` paths — single source of parity logic. `PolicyDomain::{CiGate,ArchRule,
CrlGate,AuditCheck}` and `PolicySourceKind::{Command,Guard}` already exist in the Plan 1a
model (`registry.rs:44-79`) — no model/schema change.

**Build-matrix discipline:** Task 5 Step 6 + Task 6 Step 6 build/test **both** with and
without `completion-toestub`, catching cfg-gate mistakes. `cargo fmt -p vox-cli` only
(never `--all`). Debug-binary gate runs carry `VOX_SKIP_FRESHNESS_CHECK=1`.

---

## What this defers

- **Self-registering the `policy-registry-parity` gate as a catalog op** (`ci.policy-registry-parity`
  in `operations/catalog.v1.yaml`) — must go through `vox ci capability-sync`, not a hand-edit;
  out of scope (Task 7 Step 2 NOTE). The gate enumerates it automatically once added.
- **`workflow-job` domain** (`.github/workflows/*.yml` job names, link-only) — spec lists it
  but it is the lowest-value, most-volatile source; deferred to a later slice.
- **Per-branch status overlay** (`PolicyRunReport`, `.vox/policy-status/`, `vox policy status`,
  `--json` wiring for arch/code-audit) → Plan 1c (already drafted).
- **GUI Policies surface** (nav rename, Tauri IPC, React surface) → Plan 1d.
- **Phase 2 enable/disable + protected-policy enforcement** — `protected` flags are *set*
  here (arch structural guards, GA-blocking CR-L gates, stub/llm code-audit rules) but not yet
  *enforced*; that is Phase 2.
