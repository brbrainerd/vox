//! `vox ci completion-*` — LLM premature-completion audit, gates, and VoxDB ingest (convergence SSOT).
//!
//! **`completion-ingest`** persists audit JSON into `ci_completion_*` (**S2** workspace-adjacent: paths,
//! fingerprints). Policy: `contracts/operations/completion-policy.v1.yaml`; retention:
//! `contracts/db/retention-policy.yaml`; classification SSOT: `docs/src/architecture/telemetry-retention-sensitivity-ssot.md`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use vox_cli_ci::cmd_enums::CompletionGateMode;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use vox_bounded_fs::read_utf8_path_capped;

pub const COMPLETION_POLICY_REL: &str = "contracts/operations/completion-policy.v1.yaml";
pub const COMPLETION_POLICY_SCHEMA_REL: &str =
    "contracts/operations/completion-policy.v1.schema.json";
pub const COMPLETION_AUDIT_REPORT_REL: &str = "contracts/reports/completion-audit.v1.json";
pub const COMPLETION_BASELINE_REL: &str = "contracts/reports/completion-baseline.v1.json";

/// MCP/tool static failure sentinel (concat so this source file does not self-trigger the scanner).
const STUB_RESPONSE_SENTINEL: &str = concat!("stub", "-", "response");
const DETECTOR_STUB_RESPONSE_LITERAL: &str = concat!("stub", "-response", "-literal");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub policy_schema_version: u32,
    pub repository_id: Option<String>,
    pub findings: Vec<AuditFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFinding {
    pub detector_id: String,
    pub tier: String,
    pub severity: String,
    pub file_path: Option<String>,
    pub line: Option<u32>,
    pub message: String,
    pub fingerprint: String,
    /// Numeric weight for Tier B aggregate detectors (e.g. ignore count). Defaults to 1 when absent.
    #[serde(default)]
    pub metric: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct PolicyDetector {
    id: String,
    tier: String,
}

#[derive(Debug, Deserialize)]
struct CompletionPolicyFile {
    schema_version: u32,
    detectors: Vec<PolicyDetector>,
    #[serde(default)]
    audit_exemptions: Option<AuditExemptions>,
}

#[derive(Debug, Deserialize)]
struct AuditExemptions {
    #[serde(default)]
    contract_enforced_by_empty_ids: Vec<String>,
    #[serde(default)]
    stub_response_path_suffixes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct IndexContractRow {
    id: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    enforced_by: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct IndexYaml {
    #[allow(dead_code)]
    schema_version: u32,
    contracts: Vec<IndexContractRow>,
}

#[derive(Debug, Deserialize)]
struct BaselineFile {
    #[allow(dead_code)]
    schema_version: u32,
    #[serde(default)]
    tier_b_max_by_detector: HashMap<String, i64>,
}

fn validate_policy_schema(repo_root: &Path, raw: &str) -> Result<()> {
    let schema_path = repo_root.join(COMPLETION_POLICY_SCHEMA_REL);
    let schema_val: serde_json::Value = serde_json::from_str(&read_utf8_path_capped(&schema_path)?)
        .with_context(|| format!("parse {}", schema_path.display()))?;
    let instance: serde_json::Value =
        serde_yaml::from_str(raw).context("parse completion policy as JSON value")?;
    let validator = vox_jsonschema_util::compile_validator(&schema_val, schema_path.display())
        .context("compile completion-policy schema")?;
    vox_jsonschema_util::validate(
        &instance,
        &validator,
        "completion-policy vs completion-policy.v1.schema.json",
    )
    .map_err(|e| anyhow!("{e:#}"))?;
    Ok(())
}

/// Validate completion policy YAML + JSON Schema (for `vox ci command-compliance`).
pub fn verify_policy_contract(repo_root: &Path) -> Result<()> {
    let policy_path = repo_root.join(COMPLETION_POLICY_REL);
    let raw = read_utf8_path_capped(&policy_path)
        .with_context(|| format!("read {}", policy_path.display()))?;
    validate_policy_schema(repo_root, &raw)?;
    let _: CompletionPolicyFile =
        serde_yaml::from_str(&raw).context("parse completion-policy YAML")?;
    Ok(())
}

fn fingerprint_for(detector_id: &str, path: &str, line: u32, msg: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    detector_id.hash(&mut h);
    path.hash(&mut h);
    line.hash(&mut h);
    msg.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn normalize_repo_rel(path: &Path, repo_root: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Default `crates/` plus optional `--scan-extra` dirs; all must exist and stay under `repo_root` (canonical).
pub fn resolve_audit_scan_roots(repo_root: &Path, extras: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let repo_canon = std::fs::canonicalize(repo_root)
        .with_context(|| format!("canonicalize repository root {}", repo_root.display()))?;
    let default_crates = repo_root.join("crates");
    let mut seen = HashSet::<PathBuf>::new();
    let mut out = Vec::new();
    for seed in std::iter::once(default_crates).chain(extras.iter().map(|e| {
        if e.is_absolute() {
            e.clone()
        } else {
            repo_root.join(e)
        }
    })) {
        let p = std::fs::canonicalize(&seed).with_context(|| {
            format!(
                "completion-audit scan path does not exist: {}",
                seed.display()
            )
        })?;
        if !p.starts_with(&repo_canon) {
            anyhow::bail!(
                "--scan-extra escapes repository root: {} (root {})",
                p.display(),
                repo_canon.display()
            );
        }
        if !p.is_dir() {
            anyhow::bail!("--scan-extra must be a directory: {}", p.display());
        }
        if seen.insert(p.clone()) {
            out.push(p);
        }
    }
    Ok(out)
}

fn policy_detector_tiers(policy: &CompletionPolicyFile) -> HashMap<String, String> {
    policy
        .detectors
        .iter()
        .map(|d| (d.id.clone(), d.tier.clone()))
        .collect()
}

fn policy_detector_id_set(policy: &CompletionPolicyFile) -> HashSet<String> {
    policy.detectors.iter().map(|d| d.id.clone()).collect()
}

fn tier_for_detector(tiers: &HashMap<String, String>, detector_id: &str) -> Result<String> {
    let base = detector_id
        .split_once('/')
        .map(|(a, _)| a)
        .unwrap_or(detector_id);
    tiers.get(base).cloned().ok_or_else(|| {
        anyhow!(
            "completion-policy.v1.yaml detectors[] missing id `{}` (for finding id `{}`)",
            base,
            detector_id
        )
    })
}

fn validate_findings_policy_ids(
    findings: &[AuditFinding],
    allowed: &HashSet<String>,
) -> Result<()> {
    for f in findings {
        let base = f
            .detector_id
            .split_once('/')
            .map(|(a, _)| a)
            .unwrap_or(f.detector_id.as_str());
        if !allowed.contains(base) {
            anyhow::bail!(
                "audit finding detector_id `{}` (base `{}`) not in completion-policy detectors[]",
                f.detector_id,
                base
            );
        }
    }
    Ok(())
}

fn collect_audit_findings(
    repo_root: &Path,
    scan_roots: &[PathBuf],
) -> Result<(Vec<AuditFinding>, u32)> {
    let policy_path = repo_root.join(COMPLETION_POLICY_REL);
    let raw = read_utf8_path_capped(&policy_path)
        .with_context(|| format!("read {}", policy_path.display()))?;
    validate_policy_schema(repo_root, &raw)?;
    let policy: CompletionPolicyFile =
        serde_yaml::from_str(&raw).context("parse completion-policy YAML")?;
    let tiers = policy_detector_tiers(&policy);
    let allowed_ids = policy_detector_id_set(&policy);

    let exempt_contracts: HashSet<String> = policy
        .audit_exemptions
        .as_ref()
        .map(|e| e.contract_enforced_by_empty_ids.iter().cloned().collect())
        .unwrap_or_default();
    let stub_allow: Vec<String> = policy
        .audit_exemptions
        .as_ref()
        .map(|e| e.stub_response_path_suffixes.clone())
        .unwrap_or_default();

    let mut findings: Vec<AuditFinding> = Vec::new();

    // -- contracts/index.yaml enforced_by empty (Tier A)
    let index_path = repo_root.join("contracts/index.yaml");
    let index_raw = read_utf8_path_capped(&index_path)?;
    let index: IndexYaml = serde_yaml::from_str(&index_raw).context("parse contracts index")?;
    for c in &index.contracts {
        if c.enforced_by.is_empty() {
            let desc = c.description.as_deref().unwrap_or("").to_ascii_lowercase();
            if desc.contains("draft")
                || desc.contains("placeholder")
                || desc.contains("incremental audit placeholder")
            {
                continue;
            }
            if exempt_contracts.contains(&c.id) {
                continue;
            }
            let msg = format!(
                "contracts index `{}` has enforced_by: [] (mature contracts must list an enforcer)",
                c.id
            );
            findings.push(AuditFinding {
                detector_id: "contract-index-enforced-by-empty".into(),
                tier: tier_for_detector(&tiers, "contract-index-enforced-by-empty")?,
                severity: "error".into(),
                file_path: Some("contracts/index.yaml".into()),
                line: None,
                message: msg.clone(),
                fingerprint: fingerprint_for(
                    "contract-index-enforced-by-empty",
                    "contracts/index.yaml",
                    0,
                    &msg,
                ),
                metric: None,
            });
        }
    }

    // -- MCP/static "stub response" sentinel scan (Tier A); needle built via concat! to avoid self-match.
    for scan_root in scan_roots {
        for entry in WalkDir::new(scan_root).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            if p.extension().and_then(|x| x.to_str()) != Some("rs") {
                continue;
            }
            let rel = normalize_repo_rel(p, repo_root);
            if rel.contains("/target/") {
                continue;
            }
            if stub_allow
                .iter()
                .any(|s| rel.replace('\\', "/").ends_with(s))
            {
                continue;
            }
            let text = match read_utf8_path_capped(p) {
                Ok(t) => t,
                Err(_) => continue,
            };
            for (i, line) in text.lines().enumerate() {
                if line.contains(STUB_RESPONSE_SENTINEL) {
                    let msg = format!("{STUB_RESPONSE_SENTINEL} sentinel in source: {rel}");
                    findings.push(AuditFinding {
                        detector_id: DETECTOR_STUB_RESPONSE_LITERAL.into(),
                        tier: tier_for_detector(&tiers, DETECTOR_STUB_RESPONSE_LITERAL)?,
                        severity: "error".into(),
                        file_path: Some(rel.clone()),
                        line: Some((i + 1) as u32),
                        message: msg.clone(),
                        fingerprint: fingerprint_for(
                            DETECTOR_STUB_RESPONSE_LITERAL,
                            &rel,
                            (i + 1) as u32,
                            &msg,
                        ),
                        metric: None,
                    });
                }
            }
        }
    }

    // -- ignored tests (Tier B count)
    let mut ignore_count: u64 = 0;
    for scan_root in scan_roots {
        for entry in WalkDir::new(scan_root).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if !p.is_file() || p.extension().and_then(|x| x.to_str()) != Some("rs") {
                continue;
            }
            let rel = normalize_repo_rel(p, repo_root);
            if rel.contains("/target/") {
                continue;
            }
            let text = match read_utf8_path_capped(p) {
                Ok(t) => t,
                Err(_) => continue,
            };
            ignore_count += text.matches("#[ignore").count() as u64;
        }
    }
    if ignore_count > 0 {
        let msg = format!("ignored Rust test markers (#[ignore…) count={ignore_count}");
        findings.push(AuditFinding {
            detector_id: "ignored-test-debt".into(),
            tier: tier_for_detector(&tiers, "ignored-test-debt")?,
            severity: "warning".into(),
            file_path: None,
            line: None,
            message: msg.clone(),
            fingerprint: fingerprint_for("ignored-test-debt", "workspace", 0, &msg),
            metric: Some(ignore_count as i64),
        });
    }

    // -- todo!() in non-test paths under scanned trees (Tier B count); skips **/tests/**
    let mut todo_count: u64 = 0;
    for scan_root in scan_roots {
        for entry in WalkDir::new(scan_root).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if !p.is_file() || p.extension().and_then(|x| x.to_str()) != Some("rs") {
                continue;
            }
            let rel = normalize_repo_rel(p, repo_root);
            if rel.contains("/target/") {
                continue;
            }
            if rel.contains("/tests/") || rel.ends_with("tests.rs") || rel.contains("\\tests\\") {
                continue;
            }
            let text = match read_utf8_path_capped(p) {
                Ok(t) => t,
                Err(_) => continue,
            };
            todo_count += text.matches("todo!(").count() as u64;
        }
    }
    if todo_count > 0 {
        let msg = format!("todo!() occurrences in non-test crate sources count={todo_count}");
        findings.push(AuditFinding {
            detector_id: "todo-macro-prod".into(),
            tier: tier_for_detector(&tiers, "todo-macro-prod")?,
            severity: "warning".into(),
            file_path: None,
            line: None,
            message: msg.clone(),
            fingerprint: fingerprint_for("todo-macro-prod", "workspace", 0, &msg),
            metric: Some(todo_count as i64),
        });
    }

    validate_findings_policy_ids(&findings, &allowed_ids)?;
    Ok((findings, policy.schema_version))
}

#[cfg(feature = "completion-toestub")]
fn append_toestub_findings(
    repo_root: &Path,
    findings: &mut Vec<AuditFinding>,
    scan_roots: &[PathBuf],
) -> Result<()> {
    use vox_code_audit::{
        Language, OutputFormat, Severity, ToestubConfig, ToestubEngine, ToestubRunMode,
    };

    let cfg = ToestubConfig {
        roots: scan_roots.to_vec(),
        min_severity: Severity::Info,
        format: OutputFormat::Json,
        suggest_fixes: false,
        languages: Some(vec![Language::Rust]),
        excludes: vec!["**/target/**".to_string()],
        rule_filter: Some(vec![
            "victory-claim".to_string(),
            "skeleton/hollow-fn".to_string(),
            "skeleton/declared-not-called".to_string(),
        ]),
        run_mode: ToestubRunMode::Audit,
        ..ToestubConfig::default()
    };
    let engine = ToestubEngine::new(cfg);
    let analysis = engine.run();
    for tf in analysis.findings {
        let rel = normalize_repo_rel(&tf.file, repo_root);
        let sev = match tf.severity {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error | Severity::Critical => "error",
        };
        let msg = format!("{} — {}", tf.rule_name, tf.message);
        findings.push(AuditFinding {
            detector_id: tf.rule_id.clone(),
            tier: "C".into(),
            severity: sev.into(),
            file_path: Some(rel.clone()),
            line: Some(tf.line as u32),
            message: msg.clone(),
            fingerprint: fingerprint_for(&tf.rule_id, &rel, tf.line as u32, &msg),
            metric: None,
        });
    }
    Ok(())
}

#[cfg(not(feature = "completion-toestub"))]
fn append_toestub_findings(
    _repo_root: &Path,
    _findings: &mut Vec<AuditFinding>,
    _scan_roots: &[PathBuf],
) -> Result<()> {
    let _ = std::hint::black_box((
        _repo_root.as_os_str().len(),
        _findings.len(),
        _scan_roots.len(),
    ));
    Ok(())
}

/// Build an audit report in memory (used by audit, ssot verify, and ingest).
pub fn build_audit_report(repo_root: &Path, scan_extra: &[PathBuf]) -> Result<AuditReport> {
    let scan_roots = resolve_audit_scan_roots(repo_root, scan_extra)?;
    let (mut findings, policy_schema_version) = collect_audit_findings(repo_root, &scan_roots)?;
    append_toestub_findings(repo_root, &mut findings, &scan_roots)?;

    let policy_path = repo_root.join(COMPLETION_POLICY_REL);
    let raw = read_utf8_path_capped(&policy_path)
        .with_context(|| format!("read {}", policy_path.display()))?;
    let policy: CompletionPolicyFile =
        serde_yaml::from_str(&raw).context("parse completion-policy YAML")?;
    let tiers = policy_detector_tiers(&policy);
    let allowed_ids = policy_detector_id_set(&policy);
    for f in findings.iter_mut() {
        f.tier = tier_for_detector(&tiers, &f.detector_id)?;
    }
    validate_findings_policy_ids(&findings, &allowed_ids)?;

    let repo_id = Some(vox_repository::compute_repository_id(repo_root, None));
    Ok(AuditReport {
        schema_version: 1,
        generated_at: chrono::Utc::now().to_rfc3339(),
        policy_schema_version,
        repository_id: repo_id,
        findings,
    })
}

/// Produce `contracts/reports/completion-audit.v1.json`.
pub fn run_audit(repo_root: &Path, scan_extra: &[PathBuf]) -> Result<()> {
    let report = build_audit_report(repo_root, scan_extra)?;

    let out_path = repo_root.join(COMPLETION_AUDIT_REPORT_REL);
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&report)? + "\n";
    fs::write(&out_path, json).with_context(|| format!("write {}", out_path.display()))?;
    println!(
        "completion-audit OK ({} findings) → {}",
        report.findings.len(),
        out_path.display()
    );
    Ok(())
}

/// Fail on Tier A findings only (no report write); for `vox ci ssot-drift`.
pub fn run_audit_verify_ssot(repo_root: &Path) -> Result<()> {
    let report = build_audit_report(repo_root, &[])?;
    let tier_a: Vec<_> = report.findings.iter().filter(|f| f.tier == "A").collect();
    if !tier_a.is_empty() {
        for f in &tier_a {
            eprintln!("completion-policy Tier A [{}] {}", f.detector_id, f.message);
        }
        anyhow::bail!(
            "completion-policy: {} Tier A finding(s) (run `vox ci completion-audit` for full report)",
            tier_a.len()
        );
    }
    println!(
        "ssot-drift: completion-policy scan OK ({} findings, 0 Tier A)",
        report.findings.len()
    );
    Ok(())
}

/// Apply Tier A (and optional Tier B baseline) gates to the last audit report.
pub fn run_gates(repo_root: &Path, mode: CompletionGateMode) -> Result<()> {
    let audit_path = repo_root.join(COMPLETION_AUDIT_REPORT_REL);
    let raw = read_utf8_path_capped(&audit_path).with_context(|| {
        format!(
            "read {}; run vox ci completion-audit first",
            audit_path.display()
        )
    })?;
    let report: AuditReport = serde_json::from_str(&raw).context("parse completion audit JSON")?;

    let tier_a: Vec<_> = report.findings.iter().filter(|f| f.tier == "A").collect();
    if !tier_a.is_empty() && matches!(mode, CompletionGateMode::Enforce) {
        for f in &tier_a {
            eprintln!("Tier A: [{}] {}", f.detector_id, f.message);
        }
        anyhow::bail!(
            "completion-gates: {} Tier A finding(s); fix or update policy exemptions",
            tier_a.len()
        );
    }
    if !tier_a.is_empty() && matches!(mode, CompletionGateMode::Warn) {
        println!(
            "completion-gates WARN: {} Tier A finding(s) (would fail in enforce mode)",
            tier_a.len()
        );
    }

    let baseline_path = repo_root.join(COMPLETION_BASELINE_REL);
    if baseline_path.is_file() {
        let b_raw = read_utf8_path_capped(&baseline_path)?;
        let baseline: BaselineFile =
            serde_json::from_str(&b_raw).context("parse completion baseline JSON")?;
        let mut metric_by_detector: HashMap<String, i64> = HashMap::new();
        for f in &report.findings {
            if f.tier != "B" {
                continue;
            }
            let v = f.metric.unwrap_or(1);
            *metric_by_detector.entry(f.detector_id.clone()).or_insert(0) += v;
        }

        let mut regressions = Vec::new();
        for (detector, max_allowed) in &baseline.tier_b_max_by_detector {
            let got = *metric_by_detector.get(detector).unwrap_or(&0);
            if got > *max_allowed {
                regressions.push(format!(
                    "detector {detector}: metric {got} exceeds baseline max {max_allowed}"
                ));
            }
        }

        if !regressions.is_empty() {
            for r in &regressions {
                if matches!(mode, CompletionGateMode::Enforce) {
                    eprintln!("Tier B regression: {r}");
                } else {
                    println!("completion-gates WARN: Tier B {r} (would fail in enforce mode)");
                }
            }
            if matches!(mode, CompletionGateMode::Enforce) {
                anyhow::bail!(
                    "completion-gates: {} Tier B baseline regression(s); update debt or baseline intentionally",
                    regressions.len()
                );
            }
        }
    }

    println!("completion-gates OK");
    Ok(())
}

/// Persist a completion audit JSON report into VoxDB (`ci_completion_*`).
pub async fn run_ingest(
    repo_root: &Path,
    report_path: Option<PathBuf>,
    workflow: &str,
    run_kind: &str,
) -> Result<()> {
    let audit_path = report_path.unwrap_or_else(|| repo_root.join(COMPLETION_AUDIT_REPORT_REL));
    let raw = read_utf8_path_capped(&audit_path)
        .with_context(|| format!("read {}", audit_path.display()))?;
    let report: AuditReport = serde_json::from_str(&raw).context("parse completion audit JSON")?;

    let repository_id = report
        .repository_id
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| vox_repository::compute_repository_id(repo_root, None));
    if repository_id.is_empty() {
        anyhow::bail!(
            "could not determine repository_id (run `vox ci completion-audit` from a repo checkout)"
        );
    }

    let branch = std::env::var("GITHUB_HEAD_REF")
        .or_else(|_| std::env::var("GITHUB_REF_NAME"))
        .ok();
    let commit_sha = std::env::var("GITHUB_SHA").ok();

    let tool_versions = serde_json::json!({
        "vox_cli": env!("CARGO_PKG_VERSION"),
    });
    let tool_versions_json = serde_json::to_string(&tool_versions)?;

    let db = vox_db::VoxDb::connect_default()
        .await
        .map_err(|e| anyhow!("VoxDb::connect_default: {e}"))?;

    let previous_run_id = db
        .latest_ci_completion_run_id(repository_id.as_str())
        .await
        .map_err(|e| anyhow!("latest_ci_completion_run_id: {e}"))?;
    let prev_fps_by_detector = if let Some(pid) = previous_run_id {
        db.ci_completion_fingerprints_by_detector(pid)
            .await
            .map_err(|e| anyhow!("ci_completion_fingerprints_by_detector: {e}"))?
    } else {
        HashMap::new()
    };

    let mut cur_fps_by_detector: HashMap<String, HashSet<String>> = HashMap::new();
    for f in &report.findings {
        cur_fps_by_detector
            .entry(f.detector_id.clone())
            .or_default()
            .insert(f.fingerprint.clone());
    }

    let run_id = db
        .insert_ci_completion_run(
            repository_id.as_str(),
            branch.as_deref(),
            commit_sha.as_deref(),
            workflow,
            run_kind,
            Some(tool_versions_json.as_str()),
        )
        .await
        .map_err(|e| anyhow!("insert_ci_completion_run: {e}"))?;

    for f in &report.findings {
        let meta = serde_json::json!({
            "message": f.message,
            "metric": f.metric,
        });
        let meta_json = serde_json::to_string(&meta)?;
        let line_start = f.line.map(|l| l as i64);
        db.insert_ci_completion_finding(
            run_id,
            f.detector_id.as_str(),
            f.tier.as_str(),
            f.severity.as_str(),
            None,
            f.file_path.as_deref(),
            None,
            line_start,
            line_start,
            f.fingerprint.as_str(),
            Some(meta_json.as_str()),
        )
        .await
        .map_err(|e| anyhow!("insert_ci_completion_finding: {e}"))?;
    }

    let mut snapshot_acc: HashMap<String, (String, i64)> = HashMap::new();
    for f in &report.findings {
        let add = f.metric.unwrap_or(1);
        snapshot_acc
            .entry(f.detector_id.clone())
            .and_modify(|e| {
                e.1 += add;
            })
            .or_insert_with(|| (f.tier.clone(), add));
    }
    let mut detector_ids: HashSet<String> = snapshot_acc.keys().cloned().collect();
    detector_ids.extend(prev_fps_by_detector.keys().cloned());

    for detector_id in detector_ids {
        let (tier, finding_count) = snapshot_acc.get(&detector_id).cloned().unwrap_or_else(|| {
            let t = report
                .findings
                .iter()
                .find(|x| x.detector_id == detector_id)
                .map(|x| x.tier.clone())
                .unwrap_or_else(|| "C".into());
            (t, 0_i64)
        });
        let prev_set = prev_fps_by_detector
            .get(&detector_id)
            .cloned()
            .unwrap_or_default();
        let cur_set = cur_fps_by_detector
            .get(&detector_id)
            .cloned()
            .unwrap_or_default();
        let new_count = cur_set.difference(&prev_set).count() as i64;
        let resolved_count = prev_set.difference(&cur_set).count() as i64;
        let block_state = (tier == "A" && finding_count > 0).then_some("tier_a_open");
        db.upsert_ci_completion_detector_snapshot(
            run_id,
            detector_id.as_str(),
            tier.as_str(),
            finding_count,
            new_count,
            resolved_count,
            block_state,
        )
        .await
        .map_err(|e| anyhow!("upsert_ci_completion_detector_snapshot: {e}"))?;
    }

    println!(
        "completion-ingest OK (run_id={run_id}, {} findings)",
        report.findings.len()
    );
    Ok(())
}
