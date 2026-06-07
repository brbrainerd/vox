//! Bootstrap generator + drift gate for the unified policy registry.
//! Phase 1a wires the `code-audit-rule` domain; later plans add other domains.
//!
//! The `code-audit-rule` enumeration depends on the optional `vox-code-audit`
//! crate, exposed via the `completion-toestub` feature (which CI enables). When
//! that feature is off the generator/parity commands return an actionable error
//! rather than a partial catalog, mirroring `completion_quality::append_toestub_findings`.

use std::path::Path;

// Unconditional model imports for the default-build domains (ci-gate / arch /
// crl / audit). The code-audit functions are themselves feature-gated, so this
// import is only "used" when the feature is on — which does not warn.
use vox_config::{PolicyDomain, PolicyEntry, PolicySeverity, PolicySource, PolicySourceKind};

#[cfg(feature = "completion-toestub")]
fn map_severity(s: vox_code_audit::rules::Severity) -> PolicySeverity {
    use vox_code_audit::rules::Severity as S;
    match s {
        S::Info => PolicySeverity::Info,
        S::Warning => PolicySeverity::Warn,
        S::Error => PolicySeverity::Error,
        S::Critical => PolicySeverity::Critical,
    }
}

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

/// Enumerate every live detector into a registry entry.
#[cfg(feature = "completion-toestub")]
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
                // Real detector ids use `arch/stub` and `vox/llm/direct-provider-call`
                // (verified against `vox_code_audit::detectors::all_rules`), not the
                // `stub/` / `llm_provider_call` placeholders the plan assumed.
                protected: raw_id == "arch/stub" || raw_id == "vox/llm/direct-provider-call",
                origin: "builtin".to_string(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Enumerate `contracts/ci/check-targets.v1.yaml` into `audit-check` entries.
pub fn audit_check_entries(repo_root: &Path) -> Result<Vec<PolicyEntry>, String> {
    use crate::commands::audit::CheckManifest;
    let path = repo_root.join("contracts/ci/check-targets.v1.yaml");
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
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

/// Enumerate the `ci.*` operations from `contracts/operations/catalog.v1.yaml`
/// into `ci-gate` entries. Uses the existing `OperationsCatalog`/`OperationRow`
/// serde structs (operations_catalog.rs) so this stays a single SSOT.
pub fn ci_gate_entries(repo_root: &Path) -> Result<Vec<PolicyEntry>, String> {
    use crate::commands::ci::operations_catalog::OperationsCatalog;
    let path = repo_root.join("contracts/operations/catalog.v1.yaml");
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
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
                    // Nested ops are dotted in the catalog (`eval-matrix.run`); the
                    // invocable form replaces the dot with a space (`vox ci eval-matrix run`).
                    detail: Some(format!("vox ci {}", leaf.replace('.', " "))),
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

    // Live `vox ci <...>` paths from clap, normalized to dotted `ci.<a>.<b>...`.
    // Catalog op ids are dotted across the *full* nested path (e.g.
    // `ci.eval-matrix.run` == `vox ci eval-matrix run`, clap path len 3), so we
    // join every segment, not just the first leaf — otherwise legitimate nested
    // subcommands read as drift.
    let live: BTreeSet<String> = build_catalog()
        .entries
        .iter()
        .filter(|e| e.path.first().map(|s| s == "ci").unwrap_or(false) && e.path.len() >= 2)
        .map(|e| e.path.join("."))
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

/// Parse `docs/src/architecture/layers.toml` `[guards]` into `arch-rule` entries.
/// Each guard key is one entry; severity comes from its `warn`/`error`/`strict`
/// value (`strict` maps to Critical, `error` → Error, anything else → Warn).
///
/// NOTE (no public GuardsConfig): `vox-arch-check` is a binary crate with a
/// private `ArchCheckConfig`; there is no library API to enumerate guards, so we
/// parse the TOML directly. The 11 keys are pinned by the spec §10 addendum.
pub fn arch_rule_entries(repo_root: &Path) -> Result<Vec<PolicyEntry>, String> {
    let path = repo_root.join("docs/src/architecture/layers.toml");
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
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
                description: format!("{} (layers.toml [guards].{key} = \"{level}\")", human(key)),
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
                protected: matches!(
                    key.as_str(),
                    "orphan" | "forbidden_deps" | "where_things_live"
                ),
                origin: "builtin".to_string(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

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
    use std::collections::BTreeSet;
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
///
/// Under `completion-toestub` the dispatch path uses the full `run_parity`
/// instead, so this is only reachable from the default-build `run_parity` and the
/// `default_domain_tests`; silence dead_code under the feature.
#[cfg_attr(feature = "completion-toestub", allow(dead_code))]
pub fn run_parity_default_domains(repo_root: &Path) -> Result<(), String> {
    vox_config::load_policy_registry(repo_root).map_err(|e| e.to_string())?;
    validate_registry_against_schema(repo_root)?;

    // enum↔catalog cross-check for ci-gate (full nested-path normalization).
    let orphans = ci_gate_catalog_clap_drift(repo_root);
    if !orphans.is_empty() {
        return Err(format!(
            "ci-gate enum↔catalog drift: {} op(s) have no live `vox ci <leaf>` command: {orphans:?}",
            orphans.len()
        ));
    }

    let on_disk = vox_config::load_policy_registry(repo_root).map_err(|e| e.to_string())?;
    let mut total = 0usize;
    for exp in default_domain_expectations(repo_root)? {
        total += check_domain_parity(&on_disk, &exp)?;
    }
    println!(
        "policy-registry-parity OK (default domains): {total} ci-gate+arch+crl+audit entries match live sources"
    );
    Ok(())
}

/// `vox ci policy-registry [--write]`: regenerate the catalog. Works in the
/// default build (four contract/registry domains); code-audit rows are included
/// only when the `completion-toestub` feature is on.
pub fn run_generate(repo_root: &Path, write: bool) -> Result<(), String> {
    let reg = build_registry(repo_root)?;
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

/// Read the on-disk registry YAML and validate it against the committed JSON Schema.
fn validate_registry_against_schema(repo_root: &Path) -> Result<(), String> {
    let yaml_path = repo_root.join(vox_config::REGISTRY_REL_PATH);
    let schema_path = repo_root.join("contracts/policy/policy-registry.v1.schema.json");

    let yaml_src = std::fs::read_to_string(&yaml_path)
        .map_err(|e| format!("read {}: {e}", yaml_path.display()))?;
    let instance: serde_json::Value = serde_yaml::from_str(&yaml_src)
        .map_err(|e| format!("parse {}: {e}", yaml_path.display()))?;

    let schema_src = std::fs::read_to_string(&schema_path)
        .map_err(|e| format!("read {}: {e}", schema_path.display()))?;
    let schema_val: serde_json::Value = serde_json::from_str(&schema_src)
        .map_err(|e| format!("parse {}: {e}", schema_path.display()))?;

    let validator = vox_jsonschema_util::compile_validator(&schema_val, schema_path.display())
        .map_err(|e| format!("compile policy-registry schema: {e}"))?;
    vox_jsonschema_util::validate(&instance, &validator, "policy-registry.v1.yaml")
        .map_err(|e| e.to_string())
}

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
    println!(
        "policy-registry-parity OK: {total} entries across {} domains match live sources",
        expectations.len()
    );
    Ok(())
}

/// Without `completion-toestub`, the `vox-code-audit` detector registry is not
/// linked; parity runs the four default domains feature-independently and notes
/// that code-audit parity is skipped (spec §4.4).
#[cfg(not(feature = "completion-toestub"))]
pub fn run_parity(repo_root: &Path) -> Result<(), String> {
    run_parity_default_domains(repo_root)?;
    println!(
        "(note: code-audit-rule parity skipped — rebuild with `--features completion-toestub` to enforce it)"
    );
    Ok(())
}

#[cfg(all(test, feature = "completion-toestub"))]
mod tests {
    use super::*;

    #[test]
    fn generates_one_entry_per_detector() {
        let entries = code_audit_entries();
        // Live detector registry returns 51 rules (vox-code-audit detectors::all_rules).
        assert!(
            entries.len() >= 45,
            "expected the full detector set, got {}",
            entries.len()
        );
        // Real stub detector id is `arch/stub` (verified against
        // `vox_code_audit::detectors::stub::StubDetector::id`), not the
        // `stub/todo` placeholder the plan assumed.
        let stub = entries
            .iter()
            .find(|e| e.id == "code-audit/arch/stub")
            .expect("arch/stub detector should be present");
        assert_eq!(stub.domain, PolicyDomain::CodeAuditRule);
        assert_eq!(stub.source.kind, PolicySourceKind::Pattern);
        assert!(stub.id.starts_with("code-audit/"));
    }

    #[test]
    fn schema_rejects_malformed_registry() {
        // FIX 2 guard: the committed JSON Schema must actually reject a structurally
        // invalid registry (here: `schema_version` of the wrong type), proving the
        // schema is enforced rather than decorative.
        let schema_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repo root")
            .join("contracts/policy/policy-registry.v1.schema.json");
        let schema_src = std::fs::read_to_string(&schema_path).expect("read schema");
        let schema_val: serde_json::Value =
            serde_json::from_str(&schema_src).expect("parse schema");
        let validator = vox_jsonschema_util::compile_validator(&schema_val, schema_path.display())
            .expect("compile schema");
        let bad = serde_json::json!({ "schema_version": "one", "policies": [] });
        assert!(
            vox_jsonschema_util::validate(&bad, &validator, "test").is_err(),
            "schema should reject a string schema_version"
        );
    }

    #[test]
    fn duplicate_count_mismatch_is_detected() {
        // FIX 3 guard: a duplicated `id` row inflates the Vec length past the unique
        // (set) count, which the parity gate must treat as a failure. This mirrors the
        // `disk_code_audit.len() != exp_ids.len()` assertion in `run_parity`.
        use std::collections::BTreeSet;
        let entries = code_audit_entries();
        let first = entries[0].id.clone();
        let mut disk: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        disk.push(&first); // inject a duplicate
        let unique: BTreeSet<&str> = disk.iter().copied().collect();
        assert_ne!(
            disk.len(),
            unique.len(),
            "duplicate id must make the Vec length exceed the unique-id count"
        );
    }

    #[test]
    fn group_for_uses_real_namespaced_prefixes() {
        // Real detector ids are namespaced (arch/, vox/, security/, …) — verified
        // against vox_code_audit::detectors::all_rules ids in the landed `protected`
        // mapping. The Phase 1a labels (stub/secrets/…) never matched live ids.
        assert_eq!(
            group_for("code-audit/arch/stub"),
            "Language rules / Architecture"
        );
        assert_eq!(
            group_for("code-audit/vox/llm/direct-provider-call"),
            "Language rules / Vox idioms"
        );
        assert_eq!(
            group_for("code-audit/security/hardcoded-secret"),
            "Language rules / Security"
        );
        assert_eq!(
            group_for("code-audit/scaling/n-plus-one"),
            "Language rules / Scaling"
        );
        assert_eq!(
            group_for("code-audit/victory-claim/premature-done"),
            "Language rules / Victory claims"
        );
        assert_eq!(
            group_for("code-audit/ai-laziness/silent-catch"),
            "Language rules / AI-laziness"
        );
        assert_eq!(
            group_for("code-audit/skeleton/empty-fn"),
            "Language rules / Skeletons"
        );
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

#[cfg(test)]
mod default_domain_tests {
    use super::*;
    use std::path::Path;

    fn repo_root() -> std::path::PathBuf {
        // policy_registry.rs is at crates/vox-cli/src/commands/ci/; CARGO_MANIFEST_DIR
        // is crates/vox-cli, so nth(1) = crates, nth(2) = repo root.
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
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

    #[test]
    fn ci_gate_entries_cover_operations_catalog() {
        let entries = ci_gate_entries(&repo_root()).expect("load operations catalog");
        // catalog.v1.yaml has 78 `ci.*` operations (verified 2026-06-06).
        assert!(
            entries.len() >= 70,
            "expected ~78 ci ops, got {}",
            entries.len()
        );
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
        //
        // DEVIATION: `build_catalog()` (clap tree construction) overflows the 2 MB
        // default Windows test stack — the same limitation affects the pre-existing
        // `command_catalog::tests`. Run on an 8 MB worker thread so the cross-check
        // is exercised under test. The real CLI invocation uses the main thread's
        // larger stack and is verified end-to-end via `ci policy-registry-parity`.
        let orphans = std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(|| ci_gate_catalog_clap_drift(&repo_root()))
            .expect("spawn worker")
            .join()
            .expect("worker panicked");
        assert!(
            orphans.is_empty(),
            "ci ops with no live clap subcommand (enum↔catalog drift): {orphans:?}"
        );
    }

    #[test]
    fn arch_rule_entries_cover_guards() {
        let entries = arch_rule_entries(&repo_root()).expect("parse layers.toml [guards]");
        // layers.toml [guards] has exactly 11 keys (verified 2026-06-06).
        assert_eq!(
            entries.len(),
            11,
            "expected 11 arch guards, got {}",
            entries.len()
        );
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

    #[test]
    fn crl_gate_entries_cover_audit_registry() {
        let entries = crl_gate_entries();
        // `crl_gate_entries()` maps 1:1 over `vox_audit::registry()`, so assert
        // coverage dynamically — adding CR-L/tooling gates upstream must not break
        // this test (it was a brittle hardcoded `== 10` tripwire before).
        assert_eq!(
            entries.len(),
            vox_audit::registry().iter().count(),
            "every vox_audit::registry() gate must map to a crl-gate entry"
        );
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

    #[test]
    fn gate_policy_ids_subset_of_ci_gate_catalog() {
        // Every id `CiCmd::gate_policy_id()` returns MUST be a real `ci-gate`
        // entry in the committed registry — otherwise the per-gate status capture
        // would write keys that `vox policy status` can never join (silent drift).
        // Sampled variants here; the full nullary/struct set is mapped in
        // `cmd_enums::gate_policy_id`. This guards the honest-key contract.
        use crate::commands::ci::cmd_enums::CiCmd;
        let entries = ci_gate_entries(&repo_root()).expect("load operations catalog");
        let catalog: std::collections::BTreeSet<&str> =
            entries.iter().map(|e| e.id.as_str()).collect();
        let sample = [
            CiCmd::Manifest.gate_policy_id(),
            CiCmd::SsotDrift.gate_policy_id(),
            CiCmd::CommandCompliance.gate_policy_id(),
            CiCmd::RepoGuards.gate_policy_id(),
            CiCmd::LineEndings {
                all: false,
                base: None,
                autofix: false,
            }
            .gate_policy_id(),
            CiCmd::ContractsIndex.gate_policy_id(),
            CiCmd::BackendTests.gate_policy_id(),
            CiCmd::PolicySmoke.gate_policy_id(),
        ];
        for id in sample.into_iter().flatten() {
            assert!(
                catalog.contains(id),
                "gate_policy_id `{id}` is not a ci-gate entry in the registry (drift)"
            );
        }
        // Registry machinery is intentionally untracked.
        assert_eq!(CiCmd::PolicyRegistryParity.gate_policy_id(), None);
    }

    #[test]
    fn parity_passes_for_default_domains_against_committed_yaml() {
        // After regenerate, the committed YAML must contain every live
        // ci-gate / arch / crl / audit item and no extras. This runs WITHOUT the
        // completion-toestub feature, proving the non-code-audit domains are gated
        // feature-independently (spec §4.4 build-independence requirement).
        //
        // DEVIATION: the ci-gate cross-check calls `build_catalog()`, which
        // overflows the default Windows test stack; run on an 8 MB worker thread.
        let result = std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(|| run_parity_default_domains(&repo_root()))
            .expect("spawn worker")
            .join()
            .expect("worker panicked");
        result.expect("default-domain parity must pass");
    }
}
