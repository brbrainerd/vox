# Policy Registry — Phase 1a: Catalog Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a working, read-only unified policy catalog — the registry schema, a `vox-config` loader, a bootstrap generator + drift gate seeded from the `vox-code-audit` detector domain, and `vox policy list/show/domains/groups` CLI — so any rule's contents are readable from one place.

**Architecture:** A new YAML contract `contracts/policy/policy-registry.v1.yaml` (validated by a JSON Schema) is the catalog SSOT. `vox-config` owns the lightweight typed model + loader (zero new deps — `serde_yaml` already present). `vox-cli` owns the heavy machinery: a bootstrap generator that enumerates `vox_code_audit::detectors::all_rules()` into registry entries, and a `policy-registry-parity` CI gate that fails if the registry drifts from the live detector set. This plan wires **one domain (`code-audit-rule`) end-to-end**; later plans (1b) extend the generator to CI gates / arch / CR-L, (1c) adds the per-branch status overlay, (1d) adds the GUI surface.

**Tech Stack:** Rust, `serde` + `serde_yaml`, `clap` (derive subcommands), the existing `vox_code_audit::detectors` registry. Tests via `cargo test -p <crate>`. Format with `cargo fmt -p <crate>` (never `--all` on Windows).

**Scope note:** This is Phase 1a of the initiative in
[`docs/superpowers/specs/2026-06-06-unified-policy-registry-and-governance-surface-design.md`](../specs/2026-06-06-unified-policy-registry-and-governance-surface-design.md).
Read its §10 verification addendum first — it pins the exact enumeration sources.

---

## File Structure

**Create:**
- `contracts/policy/policy-registry.v1.schema.json` — JSON Schema for the catalog.
- `contracts/policy/policy-registry.v1.yaml` — the generated catalog (committed).
- `crates/vox-config/src/policy/registry.rs` — `PolicyRegistry` model + loader.
- `crates/vox-cli/src/commands/policy/mod.rs` — `PolicyCmd` enum + `run`.
- `crates/vox-cli/src/commands/ci/policy_registry.rs` — generator + parity check.

**Modify:**
- `crates/vox-config/src/policy/mod.rs` — add `pub mod registry;`.
- `crates/vox-config/src/lib.rs` — re-export registry types.
- `crates/vox-cli/src/lib.rs` — add `Cli::Policy { cmd }` variant.
- `crates/vox-cli/src/cli_dispatch/mod.rs` — dispatch `Cli::Policy` + label.
- `crates/vox-cli/src/commands/mod.rs` — add `pub mod policy;`.
- `crates/vox-cli/src/commands/ci/cmd_enums.rs` — add `PolicyRegistry`, `PolicyRegistryParity`.
- `crates/vox-cli/src/commands/ci/run_body.rs` — dispatch the two new CI subcommands.
- `docs/src/architecture/where-things-live.md` — add the policy-catalog row.

---

## Task 1: Registry model in `vox-config`

**Files:**
- Create: `crates/vox-config/src/policy/registry.rs`
- Modify: `crates/vox-config/src/policy/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-config/src/policy/registry.rs` with only the tests module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_minimal_entry() {
        let yaml = r#"
schema_version: 1
policies:
  - id: code-audit/stub/todo
    domain: code-audit-rule
    title: TODO stub detector
    group: "Language rules / Stubs (TOESTUB)"
    description: Flags stub placeholders left in shipped code.
    severity: error
    blocking: true
    runs_on: [pre-commit, ci]
    source:
      kind: pattern
      ref: "contracts/code-audit/rules.v1.yaml#stub/todo"
      detail: "todo!()|unimplemented!()"
"#;
        let reg: PolicyRegistry = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(reg.schema_version, 1);
        assert_eq!(reg.policies.len(), 1);
        let e = &reg.policies[0];
        assert_eq!(e.id, "code-audit/stub/todo");
        assert_eq!(e.domain, PolicyDomain::CodeAuditRule);
        assert_eq!(e.severity, Some(PolicySeverity::Error));
        assert!(e.blocking);
        assert!(e.default_enabled, "default_enabled defaults to true");
        assert_eq!(e.origin, "builtin");
        assert_eq!(e.source.kind, PolicySourceKind::Pattern);
        assert_eq!(e.source.reference, "contracts/code-audit/rules.v1.yaml#stub/todo");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-config policy::registry::tests::deserializes_minimal_entry`
Expected: FAIL — `cannot find type PolicyRegistry`.

- [ ] **Step 3: Write minimal implementation**

Prepend to `crates/vox-config/src/policy/registry.rs` (above the tests module):

```rust
//! Unified policy catalog model (CI gates, language rules, audits).
//!
//! Loaded from `contracts/policy/policy-registry.v1.yaml`. See
//! `docs/superpowers/specs/2026-06-06-unified-policy-registry-and-governance-surface-design.md`.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Top-level catalog document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyRegistry {
    pub schema_version: u32,
    #[serde(default)]
    pub policies: Vec<PolicyEntry>,
}

/// One governable policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyEntry {
    pub id: String,
    pub domain: PolicyDomain,
    pub title: String,
    pub group: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<PolicySeverity>,
    #[serde(default)]
    pub blocking: bool,
    #[serde(default)]
    pub runs_on: Vec<String>,
    pub source: PolicySource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
    #[serde(default = "default_true")]
    pub default_enabled: bool,
    #[serde(default)]
    pub protected: bool,
    #[serde(default = "default_origin")]
    pub origin: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyDomain {
    CiGate,
    AuditCheck,
    CrlGate,
    CodeAuditRule,
    ArchRule,
    WorkflowJob,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicySeverity {
    Info,
    Warn,
    Error,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicySource {
    pub kind: PolicySourceKind,
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicySourceKind {
    Pattern,
    Command,
    Guard,
    Subcommand,
    Workflow,
}

fn default_true() -> bool {
    true
}
fn default_origin() -> String {
    "builtin".to_string()
}

/// Error returned when the registry cannot be loaded.
#[derive(Debug)]
pub enum PolicyRegistryError {
    Io(std::io::Error),
    Parse(serde_yaml::Error),
}

impl std::fmt::Display for PolicyRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyRegistryError::Io(e) => write!(f, "reading policy registry: {e}"),
            PolicyRegistryError::Parse(e) => write!(f, "parsing policy registry: {e}"),
        }
    }
}
impl std::error::Error for PolicyRegistryError {}

/// Canonical contract path, relative to the repo root.
pub const REGISTRY_REL_PATH: &str = "contracts/policy/policy-registry.v1.yaml";

/// Load and parse the policy registry from a repo root.
///
/// `vox-config` does not self-discover the workspace root (see
/// `VoxConfig::load_from_repo_root`); the caller passes it.
pub fn load_policy_registry(repo_root: &Path) -> Result<PolicyRegistry, PolicyRegistryError> {
    let path = repo_root.join(REGISTRY_REL_PATH);
    let text = std::fs::read_to_string(&path).map_err(PolicyRegistryError::Io)?;
    serde_yaml::from_str(&text).map_err(PolicyRegistryError::Parse)
}
```

Then add to `crates/vox-config/src/policy/mod.rs`:

```rust
pub mod registry;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-config policy::registry::tests::deserializes_minimal_entry`
Expected: PASS.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-config
git add crates/vox-config/src/policy/registry.rs crates/vox-config/src/policy/mod.rs
git commit -m "feat(vox-config): policy registry model + loader"
```

---

## Task 2: Re-export registry types + loader round-trip test

**Files:**
- Modify: `crates/vox-config/src/lib.rs`
- Test: `crates/vox-config/src/policy/registry.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `registry.rs`:

```rust
#[test]
fn load_roundtrip_from_tempdir() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let contracts = dir.path().join("contracts/policy");
    std::fs::create_dir_all(&contracts).unwrap();
    let mut f = std::fs::File::create(contracts.join("policy-registry.v1.yaml")).unwrap();
    write!(
        f,
        "schema_version: 1\npolicies:\n  - id: code-audit/stub/todo\n    domain: code-audit-rule\n    title: T\n    group: G\n    description: D\n    source:\n      kind: pattern\n      ref: r\n"
    )
    .unwrap();
    let reg = load_policy_registry(dir.path()).unwrap();
    assert_eq!(reg.policies.len(), 1);
    assert_eq!(reg.policies[0].origin, "builtin");
}
```

(`tempfile` is already a dev-dependency of `vox-config`.)

- [ ] **Step 2: Run test to verify it fails (then passes)**

Run: `cargo test -p vox-config policy::registry::tests::load_roundtrip_from_tempdir`
Expected: PASS (loader already exists from Task 1). If it fails, the loader path join is wrong — fix `REGISTRY_REL_PATH`.

- [ ] **Step 3: Add public re-exports**

In `crates/vox-config/src/lib.rs`, after the existing `pub use config::{...};` block, add:

```rust
pub use policy::registry::{
    load_policy_registry, PolicyDomain, PolicyEntry, PolicyRegistry, PolicyRegistryError,
    PolicySeverity, PolicySource, PolicySourceKind, REGISTRY_REL_PATH,
};
```

- [ ] **Step 4: Verify the crate builds**

Run: `cargo build -p vox-config`
Expected: success, no warnings about unused items.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-config
git add crates/vox-config/src/lib.rs crates/vox-config/src/policy/registry.rs
git commit -m "feat(vox-config): re-export policy registry API + loader test"
```

---

## Task 3: JSON Schema + seed contract file

**Files:**
- Create: `contracts/policy/policy-registry.v1.schema.json`
- Create: `contracts/policy/policy-registry.v1.yaml`

- [ ] **Step 1: Write the schema**

Create `contracts/policy/policy-registry.v1.schema.json`:

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Vox Policy Registry v1",
  "type": "object",
  "required": ["schema_version", "policies"],
  "properties": {
    "schema_version": { "const": 1 },
    "policies": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["id", "domain", "title", "group", "description", "source"],
        "properties": {
          "id": { "type": "string", "pattern": "^[a-z0-9-]+/.+$" },
          "domain": {
            "enum": ["ci-gate", "audit-check", "crl-gate", "code-audit-rule", "arch-rule", "workflow-job"]
          },
          "title": { "type": "string", "minLength": 1 },
          "group": { "type": "string", "minLength": 1 },
          "description": { "type": "string", "minLength": 1 },
          "severity": { "enum": ["info", "warn", "error", "critical"] },
          "blocking": { "type": "boolean" },
          "runs_on": { "type": "array", "items": { "type": "string" } },
          "source": {
            "type": "object",
            "required": ["kind", "ref"],
            "properties": {
              "kind": { "enum": ["pattern", "command", "guard", "subcommand", "workflow"] },
              "ref": { "type": "string", "minLength": 1 },
              "detail": { "type": "string" }
            },
            "additionalProperties": false
          },
          "docs": { "type": "string" },
          "default_enabled": { "type": "boolean" },
          "protected": { "type": "boolean" },
          "origin": { "enum": ["builtin", "user"] }
        },
        "additionalProperties": false
      }
    }
  },
  "additionalProperties": false
}
```

- [ ] **Step 2: Create a valid empty seed file**

Create `contracts/policy/policy-registry.v1.yaml`:

```yaml
# GENERATED by `vox ci policy-registry --write`. Do not hand-edit builtin entries.
# SSOT for the unified policy catalog. See
# docs/superpowers/specs/2026-06-06-unified-policy-registry-and-governance-surface-design.md
schema_version: 1
policies: []
```

- [ ] **Step 3: Verify it loads**

Run: `cargo test -p vox-config policy::registry` (existing tests still pass; the file is valid YAML).
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add contracts/policy/policy-registry.v1.schema.json contracts/policy/policy-registry.v1.yaml
git commit -m "feat(contracts): policy registry schema + empty seed"
```

---

## Task 4: Bootstrap generator + transfer-parity for the code-audit domain

**Files:**
- Create: `crates/vox-cli/src/commands/ci/policy_registry.rs`
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs`, `crates/vox-cli/src/commands/ci/run_body.rs`

> **Pre-check:** confirm `vox-cli` already depends on `vox-code-audit` and `vox-config`
> (`grep -n 'vox-code-audit\|vox-config' crates/vox-cli/Cargo.toml`). Both are used
> elsewhere in `vox-cli`; if a dep is missing, add it under `[dependencies]` with
> `{ workspace = true }`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli/src/commands/ci/policy_registry.rs`:

```rust
//! Bootstrap generator + drift gate for the unified policy registry.
//! Phase 1a wires the `code-audit-rule` domain; later plans add other domains.

use vox_config::{PolicyDomain, PolicyEntry, PolicySeverity, PolicySource, PolicySourceKind};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_one_entry_per_detector() {
        let entries = code_audit_entries();
        // Live detector registry returns 51 rules (vox-code-audit detectors::all_rules).
        assert!(entries.len() >= 45, "expected the full detector set, got {}", entries.len());
        let todo = entries
            .iter()
            .find(|e| e.id == "code-audit/stub/todo")
            .expect("stub/todo detector should be present");
        assert_eq!(todo.domain, PolicyDomain::CodeAuditRule);
        assert_eq!(todo.source.kind, PolicySourceKind::Pattern);
        assert!(todo.id.starts_with("code-audit/"));
    }

    #[test]
    fn every_entry_has_required_fields() {
        for e in code_audit_entries() {
            assert!(!e.title.is_empty(), "{} missing title", e.id);
            assert!(!e.group.is_empty(), "{} missing group", e.id);
            assert!(!e.description.is_empty(), "{} missing description", e.id);
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli commands::ci::policy_registry::tests`
Expected: FAIL — `cannot find function code_audit_entries`.

- [ ] **Step 3: Write the generator**

Add above the tests module in `policy_registry.rs`:

```rust
use std::path::Path;

fn map_severity(s: vox_code_audit::rules::Severity) -> PolicySeverity {
    use vox_code_audit::rules::Severity as S;
    match s {
        S::Info => PolicySeverity::Info,
        S::Warning => PolicySeverity::Warn,
        S::Error => PolicySeverity::Error,
        S::Critical => PolicySeverity::Critical,
    }
}

/// Derive a human group label from a `code-audit/<a>/<b>` id.
fn group_for(id: &str) -> String {
    let body = id.strip_prefix("code-audit/").unwrap_or(id);
    let head = body.split('/').next().unwrap_or(body);
    let label = match head {
        "stub" => "Stubs (TOESTUB)",
        "victory-claim" => "Victory claims",
        "ai-laziness" => "AI-laziness",
        "secrets" | "crypto_ban" | "env_secret_shape" | "llm_provider_call" => "Security",
        "scaling" => "Scaling",
        _ => "General",
    };
    format!("Language rules / {label}")
}

/// Enumerate every live detector into a registry entry.
pub fn code_audit_entries() -> Vec<PolicyEntry> {
    let mut out: Vec<PolicyEntry> = vox_code_audit::detectors::all_rules(None)
        .iter()
        .map(|rule| {
            let raw_id = rule.id();
            let id = format!("code-audit/{raw_id}");
            let sev = map_severity(rule.severity());
            let blocking = matches!(sev, PolicySeverity::Error | PolicySeverity::Critical);
            PolicyEntry {
                id: id.clone(),
                domain: PolicyDomain::CodeAuditRule,
                title: rule.name().to_string(),
                group: group_for(&id),
                description: rule.description().to_string(),
                severity: Some(sev),
                blocking,
                runs_on: vec!["pre-commit".into(), "pre-push".into(), "ci".into()],
                source: PolicySource {
                    kind: PolicySourceKind::Pattern,
                    reference: format!("contracts/code-audit/rules.v1.yaml#{raw_id}"),
                    detail: rule.minimal_repro().map(|s| s.to_string()),
                },
                docs: None,
                default_enabled: true,
                // Stub + security rules are policy-protected (cannot be disabled in Phase 2).
                protected: raw_id.starts_with("stub/") || raw_id == "llm_provider_call",
                origin: "builtin".to_string(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Build the full registry document for the domains this plan covers.
pub fn build_registry() -> vox_config::PolicyRegistry {
    vox_config::PolicyRegistry {
        schema_version: 1,
        policies: code_audit_entries(),
    }
}

/// `vox ci policy-registry [--write]`: regenerate the catalog.
pub fn run_generate(repo_root: &Path, write: bool) -> Result<(), String> {
    let reg = build_registry();
    let header = "# GENERATED by `vox ci policy-registry --write`. Do not hand-edit builtin entries.\n# SSOT for the unified policy catalog.\n";
    let body = serde_yaml::to_string(&reg).map_err(|e| format!("serialize: {e}"))?;
    let yaml = format!("{header}{body}");
    let path = repo_root.join(vox_config::REGISTRY_REL_PATH);
    if write {
        std::fs::write(&path, &yaml).map_err(|e| format!("write {}: {e}", path.display()))?;
        println!("wrote {} ({} policies)", path.display(), reg.policies.len());
    } else {
        println!("{yaml}");
    }
    Ok(())
}
```

> **Note on `vox_code_audit::detectors::all_rules` path:** confirm the exact public
> path with `grep -n "pub fn all_rules" crates/vox-code-audit/src/detectors/mod.rs`
> and that `detectors` is re-exported in `vox-code-audit/src/lib.rs`. If `detectors`
> is private, use the public re-export (e.g. `vox_code_audit::all_rules`). Adjust the
> two call sites accordingly — do not add a new pub surface to `vox-code-audit`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-cli commands::ci::policy_registry::tests`
Expected: PASS (both tests).

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/policy_registry.rs
git commit -m "feat(vox-cli): policy registry bootstrap generator (code-audit domain)"
```

---

## Task 5: Wire the two CI subcommands + generate the real catalog

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs`, `crates/vox-cli/src/commands/ci/run_body.rs`

- [ ] **Step 1: Add the enum variants**

In `crates/vox-cli/src/commands/ci/cmd_enums.rs`, inside `enum CiCmd`, add (follow the
existing `#[command(name = "...")]` + `///` doc style of neighboring variants):

```rust
    /// Regenerate the unified policy registry from live sources.
    #[command(name = "policy-registry")]
    PolicyRegistry {
        /// Write the registry to disk instead of printing it.
        #[arg(long)]
        write: bool,
    },
    /// Fail if the policy registry has drifted from the live detector set.
    #[command(name = "policy-registry-parity")]
    PolicyRegistryParity,
```

- [ ] **Step 2: Add the parity check function**

Append to `crates/vox-cli/src/commands/ci/policy_registry.rs` (above tests):

```rust
/// `vox ci policy-registry-parity`: assert the committed registry matches the
/// live `code-audit` detector set exactly (no drift). This is the transfer/
/// completeness proof for the domains this plan covers.
pub fn run_parity(repo_root: &Path) -> Result<(), String> {
    let on_disk = vox_config::load_policy_registry(repo_root).map_err(|e| e.to_string())?;
    let expected = build_registry();

    use std::collections::BTreeSet;
    let disk_ids: BTreeSet<&str> = on_disk
        .policies
        .iter()
        .filter(|e| e.domain == PolicyDomain::CodeAuditRule)
        .map(|e| e.id.as_str())
        .collect();
    let exp_ids: BTreeSet<&str> = expected.policies.iter().map(|e| e.id.as_str()).collect();

    let missing: Vec<&str> = exp_ids.difference(&disk_ids).copied().collect();
    let extra: Vec<&str> = disk_ids.difference(&exp_ids).copied().collect();
    if !missing.is_empty() || !extra.is_empty() {
        return Err(format!(
            "policy registry drift (code-audit domain):\n  missing {} entry(ies): {:?}\n  stale {} entry(ies): {:?}\n  run `vox ci policy-registry --write`",
            missing.len(),
            missing,
            extra.len(),
            extra
        ));
    }
    println!(
        "policy-registry-parity OK: {} code-audit entries match live detectors",
        exp_ids.len()
    );
    Ok(())
}
```

- [ ] **Step 3: Dispatch in `run_body.rs`**

In `crates/vox-cli/src/commands/ci/run_body.rs`, find the `match cmd { ... }` and add two
arms (mirror how a neighboring arm obtains `root`; the file already computes a repo root
for `enforce_for_ci`). Use that same `root` binding:

```rust
        CiCmd::PolicyRegistry { write } => {
            super::policy_registry::run_generate(&root, write).map_err(|e| anyhow::anyhow!(e))?;
        }
        CiCmd::PolicyRegistryParity => {
            super::policy_registry::run_parity(&root).map_err(|e| anyhow::anyhow!(e))?;
        }
```

> If `run_body.rs`'s error type is not `anyhow`, match the file's existing pattern
> (e.g. `.map_err(Into::into)?` or returning the `String` via the crate's error type).
> Also add `pub mod policy_registry;` to `crates/vox-cli/src/commands/ci/mod.rs` if the
> module is not auto-declared.

- [ ] **Step 4: Generate the real catalog and verify parity**

```bash
cargo run -p vox-cli -- ci policy-registry --write
cargo run -p vox-cli -- ci policy-registry-parity
```
Expected: the YAML now lists all `code-audit/*` policies; parity prints `OK`.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/run_body.rs crates/vox-cli/src/commands/ci/policy_registry.rs crates/vox-cli/src/commands/ci/mod.rs contracts/policy/policy-registry.v1.yaml
git commit -m "feat(vox-cli): policy-registry + policy-registry-parity CI gates; generate catalog"
```

---

## Task 6: `vox policy` read-only CLI

**Files:**
- Create: `crates/vox-cli/src/commands/policy/mod.rs`
- Modify: `crates/vox-cli/src/commands/mod.rs`, `crates/vox-cli/src/lib.rs`, `crates/vox-cli/src/cli_dispatch/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli/src/commands/policy/mod.rs`:

```rust
//! `vox policy` — read-only view over the unified policy catalog.

use clap::Subcommand;
use vox_config::{PolicyEntry, PolicyRegistry};

#[derive(Debug, Subcommand)]
pub enum PolicyCmd {
    /// List policies, optionally filtered by domain or group.
    List {
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        group: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show full detail (including rule contents) for one policy id.
    Show { id: String },
    /// List the distinct domains present in the catalog.
    Domains,
    /// List the distinct group labels present in the catalog.
    Groups,
}

fn matches_filter(e: &PolicyEntry, domain: &Option<String>, group: &Option<String>) -> bool {
    let dom_ok = domain
        .as_deref()
        .map(|d| format!("{:?}", e.domain).to_lowercase().replace('_', "-").contains(&d.to_lowercase()) || serde_domain(e) == d)
        .unwrap_or(true);
    let grp_ok = group
        .as_deref()
        .map(|g| e.group.to_lowercase().contains(&g.to_lowercase()))
        .unwrap_or(true);
    dom_ok && grp_ok
}

fn serde_domain(e: &PolicyEntry) -> String {
    serde_yaml::to_string(&e.domain)
        .unwrap_or_default()
        .trim()
        .trim_matches('"')
        .to_string()
}

fn render_show(e: &PolicyEntry) -> String {
    let sev = e
        .severity
        .map(|s| format!("{s:?}").to_lowercase())
        .unwrap_or_else(|| "-".into());
    format!(
        "{id}\n  {title}\n  domain:   {domain}\n  group:    {group}\n  severity: {sev}{blocking}\n  runs on:  {runs}\n  origin:   {origin}\n\n  {desc}\n\n  --- rule contents ---\n  kind:   {kind}\n  source: {source}\n  detail: {detail}\n",
        id = e.id,
        title = e.title,
        domain = serde_domain(e),
        group = e.group,
        blocking = if e.blocking { " (blocking)" } else { "" },
        runs = e.runs_on.join(", "),
        origin = e.origin,
        desc = e.description,
        kind = format!("{:?}", e.source.kind).to_lowercase(),
        source = e.source.reference,
        detail = e.source.detail.as_deref().unwrap_or("(none)"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_config::{PolicyDomain, PolicySource, PolicySourceKind};

    fn entry(id: &str, group: &str) -> PolicyEntry {
        PolicyEntry {
            id: id.into(),
            domain: PolicyDomain::CodeAuditRule,
            title: "T".into(),
            group: group.into(),
            description: "D".into(),
            severity: None,
            blocking: false,
            runs_on: vec![],
            source: PolicySource { kind: PolicySourceKind::Pattern, reference: "r".into(), detail: None },
            docs: None,
            default_enabled: true,
            protected: false,
            origin: "builtin".into(),
        }
    }

    #[test]
    fn group_filter_is_case_insensitive_substring() {
        let e = entry("code-audit/stub/todo", "Language rules / Stubs (TOESTUB)");
        assert!(matches_filter(&e, &None, &Some("stubs".into())));
        assert!(!matches_filter(&e, &None, &Some("architecture".into())));
    }

    #[test]
    fn show_includes_rule_contents() {
        let out = render_show(&entry("code-audit/stub/todo", "G"));
        assert!(out.contains("rule contents"));
        assert!(out.contains("source: r"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli commands::policy::tests`
Expected: FAIL — module not declared.

- [ ] **Step 3: Declare the module + add `run`**

Add to `crates/vox-cli/src/commands/mod.rs`:

```rust
pub mod policy;
```

Append the `run` entry point to `crates/vox-cli/src/commands/policy/mod.rs` (above tests):

```rust
/// Entry point for `vox policy <cmd>`.
pub fn run(cmd: PolicyCmd, repo_root: &std::path::Path) -> anyhow::Result<()> {
    let reg: PolicyRegistry =
        vox_config::load_policy_registry(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    match cmd {
        PolicyCmd::List { domain, group, json } => {
            let items: Vec<&PolicyEntry> = reg
                .policies
                .iter()
                .filter(|e| matches_filter(e, &domain, &group))
                .collect();
            if json {
                println!("{}", serde_json::to_string_pretty(&items)?);
            } else {
                for e in items {
                    let sev = e.severity.map(|s| format!("{s:?}").to_lowercase()).unwrap_or_else(|| "-".into());
                    println!("{:<40} [{}]{}  {}", e.id, sev, if e.blocking { " blocking" } else { "" }, e.title);
                }
            }
        }
        PolicyCmd::Show { id } => match reg.policies.iter().find(|e| e.id == id) {
            Some(e) => print!("{}", render_show(e)),
            None => anyhow::bail!("no policy with id `{id}` (try `vox policy list`)"),
        },
        PolicyCmd::Domains => {
            let mut ds: Vec<String> = reg.policies.iter().map(serde_domain).collect();
            ds.sort();
            ds.dedup();
            ds.iter().for_each(|d| println!("{d}"));
        }
        PolicyCmd::Groups => {
            let mut gs: Vec<String> = reg.policies.iter().map(|e| e.group.clone()).collect();
            gs.sort();
            gs.dedup();
            gs.iter().for_each(|g| println!("{g}"));
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-cli commands::policy::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/policy/mod.rs crates/vox-cli/src/commands/mod.rs
git commit -m "feat(vox-cli): vox policy list/show/domains/groups (read-only)"
```

---

## Task 7: Register `Cli::Policy` + dispatch

**Files:**
- Modify: `crates/vox-cli/src/lib.rs`, `crates/vox-cli/src/cli_dispatch/mod.rs`

- [ ] **Step 1: Add the top-level subcommand**

In `crates/vox-cli/src/lib.rs`, inside `pub enum Cli` (near the `Config` variant at
line ~189), add:

```rust
    /// View the unified policy catalog (CI gates, language rules, audits).
    Policy {
        #[command(subcommand)]
        cmd: commands::policy::PolicyCmd,
    },
```

- [ ] **Step 2: Add the dispatch label**

In `crates/vox-cli/src/cli_dispatch/mod.rs`, near line 48 where `Cli::Audit { .. } => Some("audit")`:

```rust
        Cli::Policy { .. } => Some("policy"),
```

- [ ] **Step 3: Add the dispatch arm**

In the main dispatch match in `cli_dispatch/mod.rs` (near `Cli::Config { cmd } => {`),
add (use the same repo-root discovery the neighboring `Cli::Audit`/`Cli::Config` arms
use — pass the discovered root):

```rust
        Cli::Policy { cmd } => {
            let root = std::env::current_dir()?;
            commands::policy::run(cmd, &root)?;
        }
```

> If neighboring arms use a shared helper (e.g. `repo_root()` / `VoxConfig` root
> discovery) rather than `current_dir()`, use that helper for consistency.

- [ ] **Step 4: End-to-end verification**

```bash
cargo run -p vox-cli -- policy domains
cargo run -p vox-cli -- policy list --group stubs
cargo run -p vox-cli -- policy show code-audit/stub/todo
```
Expected: `domains` prints `code-audit-rule`; `list --group stubs` shows the stub
detectors; `show` prints the rule contents (kind/source/detail).

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/lib.rs crates/vox-cli/src/cli_dispatch/mod.rs
git commit -m "feat(vox-cli): register vox policy subcommand"
```

---

## Task 8: Where-things-live row + final gate

**Files:**
- Modify: `docs/src/architecture/where-things-live.md`

- [ ] **Step 1: Add the lookup row**

Add a row to the table in `docs/src/architecture/where-things-live.md` (match the
existing column format):

```markdown
| Unified policy catalog (CI gates, language rules, audits) | `contracts/policy/policy-registry.v1.yaml` (SSOT); model+loader in `vox-config` (`policy::registry`); generator/parity/`vox policy` in `vox-cli` |
```

- [ ] **Step 2: Run architecture + workspace checks**

```bash
cargo run -p vox-arch-check
cargo test -p vox-config
cargo test -p vox-cli commands::policy commands::ci::policy_registry
```
Expected: arch-check green (the WTL row satisfies the where-things-live guard); all
policy tests pass.

- [ ] **Step 3: Run the new parity gate as CI would**

```bash
cargo run -p vox-cli -- ci policy-registry-parity
```
Expected: `policy-registry-parity OK`.

- [ ] **Step 4: Commit**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): where-things-live row for policy catalog"
```

---

## Self-Review

**Spec coverage (Phase 1a slice):**
- Unified registry SSOT + schema → Tasks 1, 3. ✓
- `vox-config` ownership, explicit `repo_root`, zero new deps → Tasks 1, 2. ✓
- Bootstrap generator + transfer/completeness proof (code-audit domain) → Tasks 4, 5. ✓
- `policy-registry-parity` drift gate → Task 5. ✓
- Read-the-contents CLI (`vox policy show`) → Tasks 6, 7. ✓
- where-things-live + arch-check → Task 8. ✓
- **Deferred to later plans (documented, not gaps):** other domains (ci-gate via
  operations-catalog, arch via `[guards]`, CR-L, check-targets) → Plan 1b; status
  overlay → Plan 1c; GUI surface + nav rename → Plan 1d.

**Placeholder scan:** No "TBD"/"add error handling"/bare references. The three `>`
notes (dep pre-check, `all_rules` path confirmation, dispatch error-type matching)
are verification reminders against real files, with concrete fallbacks — not
placeholders.

**Type consistency:** `PolicyEntry`/`PolicySource.reference` (serde `ref`)/
`PolicyDomain::CodeAuditRule`/`PolicySeverity::Warn` are used identically across
Tasks 1, 4, 6. `load_policy_registry(repo_root)` signature matches all call sites.
`code_audit_entries()`/`build_registry()`/`run_generate`/`run_parity` names are
consistent between Tasks 4 and 5.

---

## Follow-on plans (Phase 1, not in this plan)

- **Plan 1b** — extend the generator + parity to `ci-gate` (from
  `contracts/operations/catalog.v1.yaml` + clap `command_catalog.rs` cross-check),
  `arch-rule` (`layers.toml [guards]`), `crl-gate` (`vox-audit` registry),
  `audit-check` (`check-targets.v1.yaml`); full transfer-parity across all domains.
- **Plan 1c** — per-branch status overlay: `PolicyRunReport` type +
  `.vox/policy-status/<branch>.json` store; dispatch wrapper in `run_body.rs`
  (per-gate status) + `--json` wiring for `vox-code-audit`/`vox-arch-check`
  (per-finding); `vox policy status`; reader in `vox-config`.
- **Plan 1d** — GUI Policies surface: rename `matrix` nav label to "Routing", add
  `policies` `view_key`, Tauri IPC (`policy_list`/`policy_show`/`policy_status`/
  `list_branches`), React surface (group rail reusing `SidebarMode`, detail-dominant
  layout, graceful-empty "Needs attention", status-colored counts, master badge,
  multi-branch selector).
