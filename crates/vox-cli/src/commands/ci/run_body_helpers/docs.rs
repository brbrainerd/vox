use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use super::guards::run_sql_surface_guard;
use crate::commands::ci::cargo_bin;
use crate::commands::ci::command_compliance;
use crate::commands::ci::constants::{
    CODEX_SSOT_FILES, DOCS_SSOT_FILES, MANIFEST_SNIPPETS, OPENAPI_SUBSTRINGS,
};
use crate::commands::ci::exec_policy_contract;
use vox_bounded_fs::read_utf8_path_capped;
use vox_cli_ci::canonical_docs;
use vox_cli_ci::completion_quality;
use vox_cli_ci::contracts_index;
use vox_cli_ci::scientia_novelty_ledger_contract;
use vox_cli_ci::scientia_worthiness_contract;

pub(crate) fn run_manifest(root: &Path) -> Result<()> {
    let status = Command::new(cargo_bin())
        .current_dir(root)
        .args(["metadata", "--locked", "--format-version", "1", "--no-deps"])
        .stdout(Stdio::null())
        .status()
        .context("spawn cargo metadata")?;
    if !status.success() {
        return Err(anyhow!("cargo metadata --locked failed"));
    }
    println!("OK: workspace manifest resolves (cargo metadata --locked --no-deps)");
    Ok(())
}

pub(crate) fn check_docs_ssot(root: &Path) -> Result<()> {
    for rel in DOCS_SSOT_FILES {
        let p = root.join(rel);
        if !p.is_file() {
            return Err(anyhow!("missing: {}", p.display()));
        }
    }
    let doc_inv = root.join("docs/agents/doc-inventory.json");
    if !doc_inv.is_file() {
        return Err(anyhow!(
            "missing: {} (run: vox ci doc-inventory generate)",
            doc_inv.display()
        ));
    }
    let raw = read_utf8_path_capped(&doc_inv)?;
    let v: serde_json::Value = serde_json::from_str(&raw)?;
    let sv = v
        .get("schema_version")
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    if sv < 3 {
        return Err(anyhow!("doc-inventory.json: expected schema_version >= 3"));
    }

    let mut actual_crates = std::collections::HashSet::new();
    let inv = root.join("docs/src/architecture/orphan-surface-inventory.md");
    if inv.is_file() {
        let inv_text = read_utf8_path_capped(&inv)?;
        if !inv_text.contains("workspace-crates-start") {
            return Err(anyhow!(
                "orphan inventory: missing workspace-crates-start marker"
            ));
        }
        if !inv_text.contains("workspace-crates-end") {
            return Err(anyhow!(
                "orphan inventory: missing workspace-crates-end marker"
            ));
        }

        let listed = parse_workspace_crate_block(&inv_text);
        let crates_dir = root.join("crates");
        for entry in
            fs::read_dir(&crates_dir).with_context(|| format!("read {}", crates_dir.display()))?
        {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let toml = entry.path().join("Cargo.toml");
            if !toml.is_file() {
                continue;
            }
            let name = read_package_name(&toml)?;
            actual_crates.insert(name.clone());
            if !listed.contains(&name) {
                return Err(anyhow!(
                    "orphan inventory workspace crate list missing: {name} (from {})",
                    toml.display()
                ));
            }
        }

        for listed_crate in &listed {
            if !actual_crates.contains(listed_crate) {
                return Err(anyhow!(
                    "orphan inventory workspace crate list contains stale entry: {listed_crate} is not found in crates/*/Cargo.toml"
                ));
            }
        }
    } else {
        let crates_dir = root.join("crates");
        for entry in fs::read_dir(&crates_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let toml = entry.path().join("Cargo.toml");
                if toml.is_file() {
                    if let Ok(name) = read_package_name(&toml) {
                        actual_crates.insert(name);
                    }
                }
            }
        }
    }

    let api_dir = root.join("docs/src/api");
    if api_dir.is_dir() {
        for entry in fs::read_dir(&api_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    // Check if the filename looks like a crate name or matches an existing crate
                    if stem.starts_with("vox-") || actual_crates.contains(stem) {
                        let md = read_utf8_path_capped(&path)?;
                        let is_deprecated = md.contains("status: deprecated");
                        let is_live = actual_crates.contains(stem);

                        // Treat special aliases/clusters cautiously, but if it doesn't exist and isn't deprecated, error in DOC-036
                        // Using a simple heuristic: if it looks like a crate and isn't live, expect deprecation.
                        // Wait, some files might just be `vox-mcp.md` (live). Some might be `vox-gamify.md` (deprecated).
                        // There may be exception files like `vox-sandbox.md`. Let's just strictly enforce.
                        if !is_live && !is_deprecated {
                            // Let's verify whether the title or content specifically claims to represent a crate by checking if stem matches actual crates
                            // actually `!is_live` implies it's NOT an actual crate. So it's a phantom crate page!
                            // Exempting DOC_GAPS.md explicitly.
                            if stem != "DOC_GAPS" {
                                return Err(anyhow!(
                                    "zombie API stub {}: crate {} is not a live workspace member and status is not deprecated",
                                    path.display(),
                                    stem
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    let arch_idx_path = root.join("docs/src/architecture/architecture-index.md");
    if arch_idx_path.is_file() {
        let arch_idx_text = read_utf8_path_capped(&arch_idx_path)?;
        let arch_dir = root.join("docs/src/architecture");
        for entry in fs::read_dir(&arch_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if stem == "architecture-index" {
                        continue;
                    }
                    let md = read_utf8_path_capped(&path)?;
                    if md.contains("\nstatus: current") || md.contains("\nstatus: \"current\"") {
                        if !arch_idx_text.contains(stem) {
                            return Err(anyhow!(
                                "unlinked authority page: {} has 'status: current' but is not mentioned in {}",
                                path.display(),
                                arch_idx_path.display()
                            ));
                        }
                    }
                }
            }
        }
    }

    let mut src_files = Vec::new();
    let src_dir = root.join("docs/src");
    if src_dir.is_dir() {
        collect_text_files_under(&src_dir, &mut src_files)?;
        let now = chrono::Utc::now().naive_utc();

        for p in src_files {
            if p.extension().is_some_and(|e| e == "md") {
                if let Ok(md) = read_utf8_path_capped(&p) {
                    // Enforcement 1: Location Compliance
                    if (md.contains("status: archived") || md.contains("status: \"archived\""))
                        && !p.to_string_lossy().contains("archive")
                    {
                        return Err(anyhow!(
                            "File {} has 'status: archived' but is not in /archive/ or /docs/src/archive/",
                            p.display()
                        ));
                    }

                    if md.contains("\ntraining_eligible: true")
                        || md.contains("\ntraining_eligible: \"true\"")
                    {
                        if let Some(pos) = md.find("\nlast_updated: ") {
                            let start = pos + 15;
                            let end = start + 10;
                            if end <= md.len() {
                                let date_str = &md[start..end];
                                if let Ok(dt) =
                                    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                                {
                                    if now.date().signed_duration_since(dt).num_days() > 90 {
                                        let rel = p.strip_prefix(root).unwrap_or(&p);
                                        println!(
                                            "::warning file={},line=1::Stale training_eligible page: last_updated {} is over 90 days ago.",
                                            rel.display(),
                                            date_str
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    check_stale_doc_and_workflow_refs(root)?;
    check_archival_pipeline(root)?;
    canonical_docs::run(root)?;

    println!("Docs SSOT guard OK");
    Ok(())
}

fn check_archival_pipeline(root: &Path) -> Result<()> {
    let mut archive_files = Vec::new();
    let archive_doc_dir = root.join("docs/src/archive");
    let archive_root_dir = root.join("archive");

    if archive_doc_dir.is_dir() {
        collect_text_files_under(&archive_doc_dir, &mut archive_files)?;
    }
    if archive_root_dir.is_dir() {
        collect_text_files_under(&archive_root_dir, &mut archive_files)?;
    }

    let mut total_count = 0;
    let mut oldest_date: Option<chrono::NaiveDate> = None;

    for p in &archive_files {
        total_count += 1;
        let md = read_utf8_path_capped(p).unwrap_or_default();

        // Enforcement 2: Metadata Compliance
        if p.extension().is_some_and(|e| e == "md") {
            if !md.contains("training_eligible: false")
                && !md.contains("training_eligible: \"false\"")
            {
                return Err(anyhow!(
                    "Archived file {} must have 'training_eligible: false'",
                    p.display()
                ));
            }

            if let Some(pos) = md.find("archived_date: ") {
                let start = pos + 15;
                let end = start + 10;
                if end <= md.len() {
                    let date_str = &md[start..end];
                    if let Ok(dt) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                        if oldest_date.is_none_or(|old| dt < old) {
                            oldest_date = Some(dt);
                        }
                    } else {
                        return Err(anyhow!(
                            "Archived file {} has invalid archived_date format (expected YYYY-MM-DD)",
                            p.display()
                        ));
                    }
                }
            } else {
                return Err(anyhow!(
                    "Archived markdown file {} missing 'archived_date: YYYY-MM-DD'",
                    p.display()
                ));
            }
        }
    }

    if total_count > 0 {
        let oldest_str = oldest_date
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        println!("Archive Status: {total_count} files, oldest is {oldest_str}");
    }

    Ok(())
}

/// Fail if docs or GitHub workflows reference retired Python inventory paths or shell gates.
fn check_stale_doc_and_workflow_refs(root: &Path) -> Result<()> {
    const WORKFLOW_BANNED: &[&str] = &["verify_doc_inventory_fresh.py", "populi_release_gate.sh"];
    const DOC_BANNED: &[&str] = &[
        "verify_doc_inventory_fresh.py",
        "populi_release_gate.sh",
        // Bare form (no "crates/" prefix) so both "crates/vox-secrets/src/spec.rs"
        // and prose references like "`vox-secrets/src/spec.rs`" are caught -- the
        // bare form is a substring of the prefixed form, so this single entry
        // subsumes both.
        "vox-secrets/src/spec.rs",
        "crates/vox-orchestrator/src/mcp_tools/tools/mod.rs",
        "scripts/quality/toestub_scoped.sh",
        "four `.mdc` rule files",
    ];
    // Retired crate paths / broken SSOT links — see `docs/src/archive/research-2026-q1/nomenclature-migration-map.md`.
    // Note: "crates/vox-ml-cli/" is intentionally NOT banned here — vox-ml-cli is a
    // grandfathered real crate and implementation plan docs legitimately reference its file paths.
    const NOMENCLATURE_DOC_BANNED: &[&str] = &[
        "reference/mens.md",
        "reference/mens-ssot.md",
        "crates/vox-codex-api/",
    ];
    const DOC_PATH_BANNED: &[&str] = &["docs/how-to-ai-agents.md", "docs/src/how-to-ai-agents.md"];
    // Root-level and non-doc-tree files fixed in Tasks 1-4 that `collect_text_files_under`
    // never reaches (it only walks `docs/`) -- pushed explicitly, same pattern as
    // README.md/AGENTS.md/CONTRIBUTING.md below.
    const EXTRA_BANNED_SCAN_TARGETS: &[&str] = &[
        "GEMINI.md",
        "CLAUDE.md",
        ".github/copilot-instructions.md",
        ".cursor/rules/secrets-policy.mdc",
        "crates/vox-code-audit/src/detectors/env_secret_shape.rs",
        "crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs",
        "crates/vox-cli/src/commands/diagnostics/mod.rs",
    ];
    // Historical/point-in-time docs -- implementation plans and specs under
    // `docs/superpowers/` and archived research under `docs/src/archive/` legitimately
    // quote retired paths verbatim (before/after diff text, historical audit findings).
    // They are not living guidance, so they're exempt from this scan (Task 14's final
    // sweep applies the same exemption for the same reason).
    const EXEMPT_DOC_DIRS: &[&str] = &["docs/superpowers", "docs/src/archive"];

    let wf_dir = root.join(".github/workflows");
    if wf_dir.is_dir() {
        for entry in fs::read_dir(&wf_dir).with_context(|| format!("read {}", wf_dir.display()))? {
            let entry = entry?;
            let p = entry.path();
            if p.extension().and_then(|x| x.to_str()) != Some("yml")
                && p.extension().and_then(|x| x.to_str()) != Some("yaml")
            {
                continue;
            }
            let text = read_utf8_path_capped(&p)?;
            for b in WORKFLOW_BANNED {
                if text.contains(b) {
                    return Err(anyhow!(
                        "{}: stale or retired reference {:?} (use `vox ci` guards; see docs/src/ci/doc-inventory-ssot.md)",
                        p.display(),
                        b
                    ));
                }
            }
        }
    }

    let docs_dir = root.join("docs");
    if docs_dir.is_dir() {
        let mut files = Vec::new();
        collect_text_files_under(&docs_dir, &mut files)?;
        for rel in ["README.md", "AGENTS.md", "CONTRIBUTING.md"] {
            let p = root.join(rel);
            if p.is_file() {
                files.push(p);
            }
        }
        for rel in EXTRA_BANNED_SCAN_TARGETS {
            let p = root.join(rel);
            if p.is_file() {
                files.push(p);
            }
        }
        for p in files {
            let rel = p.strip_prefix(root).unwrap_or(&p);
            if EXEMPT_DOC_DIRS
                .iter()
                .any(|d| rel.starts_with(Path::new(d)))
            {
                continue;
            }
            // docs/src/reference/changelog.md is synced verbatim from the
            // repository-root CHANGELOG.md. A changelog names removed files by
            // construction -- the entry that records a deletion has to say what
            // was deleted -- so a "stale reference" there is the record working,
            // not drift. Same carve-out as retired_symbol_check's.
            if rel == Path::new("docs/src/reference/changelog.md") {
                continue;
            }
            let ext = p.extension().and_then(|x| x.to_str());
            if ext != Some("md")
                && ext != Some("yml")
                && ext != Some("yaml")
                && ext != Some("mdc")
                && ext != Some("rs")
            {
                continue;
            }
            let text = read_utf8_path_capped(&p)?;
            for b in DOC_BANNED {
                if text.contains(b) {
                    return Err(anyhow!(
                        "{}: stale reference {:?} — removed from tree; update docs",
                        p.display(),
                        b
                    ));
                }
            }
            for b in NOMENCLATURE_DOC_BANNED {
                if text.contains(b) {
                    return Err(anyhow!(
                        "{}: nomenclature drift {:?} — use canonical crate paths (see docs/src/archive/research-2026-q1/nomenclature-migration-map.md)",
                        p.display(),
                        b
                    ));
                }
            }
            for b in DOC_PATH_BANNED {
                if text.contains(b) {
                    return Err(anyhow!(
                        "{}: stale docs path {:?} — link the canonical mdBook path instead",
                        p.display(),
                        b
                    ));
                }
            }
        }
    }

    println!("stale doc/workflow ref scan OK");
    Ok(())
}

fn collect_text_files_under(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let p = entry.path();
        let t = entry.file_type()?;
        if t.is_dir() {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "target" || name == ".git" || name == "book" || name == "theme" {
                continue;
            }
            collect_text_files_under(&p, out)?;
        } else if t.is_file() {
            out.push(p);
        }
    }
    Ok(())
}

static CRATE_LINE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[a-z0-9_-]+$").expect("CRATE_LINE_RE"));

fn parse_workspace_crate_block(md: &str) -> std::collections::HashSet<String> {
    use std::collections::HashSet;
    let mut out = HashSet::new();
    let mut in_block = false;
    for line in md.lines() {
        let t = line.trim_end();
        if t.contains("workspace-crates-start") {
            in_block = true;
            continue;
        }
        if t.contains("workspace-crates-end") {
            in_block = false;
            continue;
        }
        if in_block {
            let s = t.trim();
            if CRATE_LINE_RE.is_match(s) {
                out.insert(s.to_string());
            }
        }
    }
    out
}

fn read_package_name(toml_path: &Path) -> Result<String> {
    let text = read_utf8_path_capped(toml_path)?;
    let v: toml::Value =
        toml::from_str(&text).with_context(|| format!("parse TOML {}", toml_path.display()))?;
    v.get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("could not read package.name from {}", toml_path.display()))
}

fn verify_baseline_policy_alignment(root: &Path) -> Result<()> {
    let policy_path = root.join("contracts/db/baseline-version-policy.yaml");
    let raw = read_utf8_path_capped(&policy_path)
        .with_context(|| format!("read {}", policy_path.display()))?;
    let v: serde_yaml::Value =
        serde_yaml::from_str(&raw).with_context(|| format!("parse {}", policy_path.display()))?;
    let expected = v
        .get("policy")
        .and_then(|p| p.get("repository_baseline_integer"))
        .and_then(|x| x.as_i64())
        .ok_or_else(|| {
            anyhow!(
                "{}: missing policy.repository_baseline_integer",
                policy_path.display()
            )
        })?;
    let manifest_path = root.join("crates/vox-db/src/schema/manifest.rs");
    let man = read_utf8_path_capped(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;
    let re = regex::Regex::new(r"pub const BASELINE_VERSION:\s*i64\s*=\s*(\d+)")
        .expect("BASELINE_VERSION parse regex");
    let got = man
        .lines()
        .find_map(|line| {
            re.captures(line)
                .and_then(|c| c.get(1)?.as_str().parse::<i64>().ok())
        })
        .ok_or_else(|| {
            anyhow!(
                "could not parse BASELINE_VERSION from {}",
                manifest_path.display()
            )
        })?;
    if got != expected {
        return Err(anyhow!(
            "baseline mismatch: {} has repository_baseline_integer={expected}, {} has BASELINE_VERSION={got}",
            policy_path.display(),
            manifest_path.display()
        ));
    }
    let digest_expected = v
        .get("policy")
        .and_then(|p| p.get("repository_baseline_digest_hex"))
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(ed) = digest_expected {
        let digest_got = vox_db::schema::schema_baseline_digest_hex();
        if digest_got != ed {
            // Auto-update logic: suggest a follow-up PR via a CI bot rather than a hard failure that blocks all PRs.
            let rel = policy_path.strip_prefix(root).unwrap_or(&policy_path);
            println!(
                "::warning file={},line=1::baseline digest mismatch: policy.repository_baseline_digest_hex={ed:?}, vox_db baseline_sql Keccak256={digest_got:?}. An auto-drafted PR should update this contract rather than blocking the current PR.",
                rel.display()
            );
        }
    }
    Ok(())
}

pub(crate) fn run_ssot_drift(root: &Path) -> Result<()> {
    // ponytail: local macro for per-sub-step timing; ssot-drift is the slowest gate (~35s)
    macro_rules! ds {
        ($label:literal, $expr:expr) => {{
            let _t = std::time::Instant::now();
            let _r = $expr;
            eprintln!("ssot-drift  {:>6}ms  {}", _t.elapsed().as_millis(), $label);
            _r
        }};
    }
    ds!("check_docs_ssot", check_docs_ssot(root))?;
    ds!("check_codex_ssot", check_codex_ssot(root))?;
    // Full-workspace scan; transitional allowlist in docs/agents/sql-connection-api-allowlist.txt
    ds!("run_sql_surface_guard", run_sql_surface_guard(root, true))?;
    ds!(
        "run_query_all_guard",
        super::guards::run_query_all_guard(root, true)
    )?;
    ds!(
        "run_turso_import_guard",
        super::guards::run_turso_import_guard(root, true)
    )?;
    ds!(
        "policy_allowlist_parity",
        crate::commands::ci::policy_allowlist_parity::run(root)
    )?;
    ds!(
        "db_schema_coverage",
        vox_cli_ci::db_schema_coverage::run(root)
    )?;
    ds!(
        "nomenclature_guard",
        vox_cli_ci::nomenclature_guard::run(root, false)
    )?;
    ds!(
        "operations_catalog::verify",
        crate::commands::ci::operations_catalog::verify(root)
    )?;
    ds!(
        "mcp_vox_surface_parity",
        vox_cli_ci::mcp_vox_surface_parity::run()
    )?;
    ds!("command_compliance", command_compliance::run(root))?;
    ds!(
        "gui_version_sync",
        vox_cli_ci::gui_version_sync::run(root, false)
    )?;
    ds!(
        "gui_catalog_parity",
        crate::commands::ci::gui_catalog_parity::run(root)
    )?;
    ds!(
        "gui_surface_coverage",
        crate::commands::ci::gui_surface_coverage::run(root, false)
    )?;
    ds!(
        "gui_surface_registry",
        crate::commands::ci::gui_surface_registry::run(root, false)
    )?;
    ds!(
        "capability_sync",
        crate::commands::ci::capability_sync::run(root, false)
    )?;
    ds!(
        "plugin_surface",
        vox_cli_ci::plugin_surface::run(root, false)
    )?;
    ds!(
        "plugin_catalog_sync",
        vox_cli_ci::plugin_catalog_sync::run(root, false)
    )?;
    // Catalog-derived reference docs must stay fresh when catalog.toml changes; this check
    // otherwise lives only in docs-quality.yml, so catalog edits could pass ssot-drift while
    // leaving the generated docs stale (the exact drift PR #218 left on main).
    ds!(
        "generate_plugin_catalog_docs",
        vox_cli_ci::generate_plugin_catalog_docs::run(None, None, true)
    )?;
    ds!(
        "plugin_skill_parity",
        vox_cli_ci::plugin_skill_parity::run(false)
    )?;
    ds!("contracts_index", contracts_index::run(root))?;
    ds!(
        "docs_reality_audit",
        vox_cli_ci::docs_reality_audit::run_verify(root)
    )?;
    ds!("exec_policy_contract", exec_policy_contract::run(root))?;
    ds!(
        "completion_quality",
        completion_quality::run_audit_verify_ssot(root)
    )?;
    ds!(
        "scientia_worthiness_contract",
        scientia_worthiness_contract::run(root)
    )?;
    ds!(
        "scientia_novelty_ledger_contract",
        scientia_novelty_ledger_contract::run(root)
    )?;
    ds!("run_data_ssot_guards", super::run_data_ssot_guards(root))?;
    ds!(
        "affected_cmd::check_graph",
        vox_cli_ci::affected_cmd::check_graph(
            root.join("contracts/ci/crate-graph.v1.json").as_path()
        )
        .map_err(|e| anyhow::anyhow!(e))
    )?;
    // ENFORCED on the CI side: an unregistered GitHub-hosted `runs-on` fails ssot-drift
    // (and therefore CI). ssot-drift is reachable from the fast pre-push tier; that
    // propagation is intentional and bounded. The standalone pre-push step
    // (pre_push.rs step_runner_policy_check) stays ADVISORY so the gate is never
    // silently bypassed by the known stale-binary `--no-verify` pattern.
    ds!(
        "runner_policy_check",
        vox_cli_ci::runner_policy_check::run(root, true)
    )?;
    ds!(
        "node_pnpm_ssot_guard",
        vox_cli_ci::node_pnpm_ssot_guard::run(root)
    )?;
    println!("ssot-drift: nested SSOT guards OK");
    Ok(())
}

pub(crate) fn check_codex_ssot(root: &Path) -> Result<()> {
    for rel in CODEX_SSOT_FILES {
        let p = root.join(rel);
        if !p.is_file() {
            return Err(anyhow!("missing: {}", p.display()));
        }
    }
    let m = root.join("crates/vox-db/src/schema/manifest.rs");
    let manifest = read_utf8_path_capped(&m).with_context(|| format!("read {}", m.display()))?;
    for needle in MANIFEST_SNIPPETS {
        if !manifest.contains(needle) {
            return Err(anyhow!("{} must contain substring: {needle}", m.display()));
        }
    }
    verify_baseline_policy_alignment(root)?;
    let o = root.join("contracts/codex-api.openapi.yaml");
    let o_text = read_utf8_path_capped(&o)?;
    for needle in OPENAPI_SUBSTRINGS {
        if !o_text.contains(needle) {
            return Err(anyhow!("openapi guard failed: missing {needle}"));
        }
    }
    println!("Codex SSOT doc guard OK");
    Ok(())
}

#[cfg(test)]
mod stale_ref_guard_tests {
    use super::*;
    use std::fs;

    fn write_agents_md(root: &Path, body: &str) {
        // check_stale_doc_and_workflow_refs only scans the root AGENTS.md/README.md/
        // CONTRIBUTING.md branch when `docs/` exists (see the `if docs_dir.is_dir()`
        // guard around that whole block) -- an empty docs/ dir is enough to enter it.
        fs::create_dir_all(root.join("docs")).expect("create docs dir");
        fs::write(root.join("AGENTS.md"), body).expect("write AGENTS.md");
    }

    #[test]
    fn flags_stale_secrets_spec_rs_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_agents_md(
            tmp.path(),
            "Define secrets in `crates/vox-secrets/src/spec.rs`.\n",
        );
        let err = check_stale_doc_and_workflow_refs(tmp.path())
            .expect_err("stale spec.rs path must be flagged");
        assert!(err.to_string().contains("vox-secrets/src/spec.rs"));
    }

    #[test]
    fn flags_stale_secrets_spec_rs_path_bare_form() {
        // Regression test: code-review found real-world docs (e.g.
        // docs/src/architecture/multi-agent-vcs-replication-spec-2026.md) reference
        // the retired path without the "crates/" prefix -- the banned string must
        // catch this bare form too, not just the fully-prefixed one.
        let tmp = tempfile::tempdir().expect("tempdir");
        write_agents_md(
            tmp.path(),
            "agents can't edit `vox-secrets/src/spec.rs` without human review\n",
        );
        let err = check_stale_doc_and_workflow_refs(tmp.path())
            .expect_err("bare-form stale spec.rs path must be flagged too");
        assert!(err.to_string().contains("vox-secrets/src/spec.rs"));
    }

    #[test]
    fn flags_stale_tool_registry_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_agents_md(
            tmp.path(),
            "See `crates/vox-orchestrator/src/mcp_tools/tools/mod.rs` for TOOL_REGISTRY.\n",
        );
        let err = check_stale_doc_and_workflow_refs(tmp.path())
            .expect_err("stale mcp_tools/tools/mod.rs path must be flagged");
        assert!(err.to_string().contains("mcp_tools/tools/mod.rs"));
    }

    #[test]
    fn flags_stale_toestub_scoped_sh() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_agents_md(tmp.path(), "Run `scripts/quality/toestub_scoped.sh`.\n");
        let err = check_stale_doc_and_workflow_refs(tmp.path())
            .expect_err("stale toestub_scoped.sh reference must be flagged");
        assert!(err.to_string().contains("toestub_scoped.sh"));
    }

    #[test]
    fn flags_stale_mdc_count_phrase() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_agents_md(tmp.path(), "There are four `.mdc` rule files here.\n");
        let err = check_stale_doc_and_workflow_refs(tmp.path())
            .expect_err("stale 'four .mdc rule files' phrase must be flagged");
        assert!(err.to_string().contains("four `.mdc` rule files"));
    }

    #[test]
    fn clean_agents_md_passes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_agents_md(
            tmp.path(),
            "Define secrets in `crates/vox-secrets/src/spec/`. Run `vox ci toestub-scoped`. Nine `.mdc` rule files.\n",
        );
        check_stale_doc_and_workflow_refs(tmp.path())
            .expect("clean doc content must pass the stale-ref guard");
    }

    #[test]
    fn ignores_historical_docs_dirs() {
        for rel in [
            "docs/superpowers/plans/example-plan.md",
            "docs/src/archive/example.md",
        ] {
            let tmp = tempfile::tempdir().expect("tempdir");
            let target = tmp.path().join(rel);
            fs::create_dir_all(target.parent().expect("has parent")).expect("create parent dir");
            fs::write(
                &target,
                "Define secrets in `crates/vox-secrets/src/spec.rs`.\n",
            )
            .expect("write file");
            check_stale_doc_and_workflow_refs(tmp.path()).unwrap_or_else(|e| {
                panic!("historical docs under {rel} must be exempt from the stale-ref scan: {e}")
            });
        }
    }

    #[test]
    fn flags_stale_path_in_newly_covered_non_doc_files() {
        for rel in [
            ".cursor/rules/secrets-policy.mdc",
            "crates/vox-code-audit/src/detectors/env_secret_shape.rs",
            "crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs",
            "crates/vox-cli/src/commands/diagnostics/mod.rs",
        ] {
            let tmp = tempfile::tempdir().expect("tempdir");
            fs::create_dir_all(tmp.path().join("docs")).expect("create docs dir");
            let target = tmp.path().join(rel);
            fs::create_dir_all(target.parent().expect("has parent")).expect("create parent dir");
            fs::write(
                &target,
                "See crates/vox-secrets/src/spec.rs for the registry.\n",
            )
            .expect("write file");
            let err = check_stale_doc_and_workflow_refs(tmp.path())
                .expect_err("stale reference in newly-covered non-doc file must be flagged");
            assert!(
                err.to_string().contains("vox-secrets/src/spec.rs"),
                "unexpected error for {rel}: {err}"
            );
        }
    }

    #[test]
    fn flags_stale_path_in_each_newly_covered_root_file() {
        for filename in ["CLAUDE.md", "GEMINI.md", ".github/copilot-instructions.md"] {
            let tmp = tempfile::tempdir().expect("tempdir");
            fs::create_dir_all(tmp.path().join("docs")).expect("create docs dir");
            let target = tmp.path().join(filename);
            fs::create_dir_all(target.parent().expect("has parent")).expect("create parent dir");
            fs::write(
                &target,
                "Define secrets in `crates/vox-secrets/src/spec.rs`.\n",
            )
            .expect("write file");
            let err = check_stale_doc_and_workflow_refs(tmp.path())
                .expect_err("stale reference in newly-covered root file must be flagged");
            assert!(
                err.to_string().contains("vox-secrets/src/spec.rs"),
                "unexpected error for {filename}: {err}"
            );
        }
    }

    #[test]
    fn skips_root_files_when_docs_dir_absent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(
            tmp.path().join("AGENTS.md"),
            "Define secrets in `crates/vox-secrets/src/spec.rs`.\n",
        )
        .expect("write AGENTS.md");
        check_stale_doc_and_workflow_refs(tmp.path()).expect(
            "without docs/, the stale-ref guard does not scan root AGENTS.md at all (documented gap)",
        );
    }
}
