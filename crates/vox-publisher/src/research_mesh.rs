//! On-disk intake for SCIENTIA [`ResearchEvent`] mesh handoff (orchestrator → publisher).
//!
//! JSON files validate against `contracts/scientia/research-mesh-intake.v1.schema.json`.
//! Valid files can be promoted into `.vox/scientia/research-mesh-promoted/events.v1.jsonl`
//! via [`consume_pending_intake`] or the background [`spawn_research_mesh_intake_consumer`] loop.

use std::fs;
use std::io::{Error, ErrorKind, Result, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use serde::Serialize;
use serde_json::json;
use serde_json::Value;

fn received_at_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn sanitize_filename_component(raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() {
        return "unknown".to_string();
    }
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    out.chars().take(128).collect()
}

fn atomic_write_json(path: &Path, payload: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, payload)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[derive(Debug, Serialize)]
struct FindingProposedIntakeV1<'a> {
    schema_version: i32,
    kind: &'static str,
    finding_id: &'a str,
    session_id: &'a str,
    claim_ids: &'a [u64],
    worthiness_score: f64,
    received_at_ms: i64,
}

#[derive(Debug, Serialize)]
struct PublicationSucceededIntakeV1<'a> {
    schema_version: i32,
    kind: &'static str,
    manifest_id: &'a str,
    received_at_ms: i64,
    doi: Option<&'a str>,
    nanopub_uris: &'a [String],
}

#[derive(Debug, Serialize)]
struct PublicationFailedIntakeV1<'a> {
    schema_version: i32,
    kind: &'static str,
    manifest_id: &'a str,
    received_at_ms: i64,
    error: &'a str,
}

/// Root intake directory for the repository (`.vox/scientia/research-mesh-intake`).
#[must_use]
pub fn mesh_intake_root(repo_root: &Path) -> PathBuf {
    vox_config::paths::repo_scientia_research_mesh_intake_dir(repo_root)
}

/// Persist a `FindingCandidateProposed`-style mesh signal for downstream publisher ingestion.
pub fn record_finding_candidate_proposed(
    repo_root: &Path,
    finding_id: &str,
    session_id: &str,
    claim_ids: &[u64],
    worthiness_score: f64,
) -> Result<PathBuf> {
    let root = mesh_intake_root(repo_root).join("finding-proposed");
    let stem = sanitize_filename_component(finding_id);
    let path = root.join(format!("{stem}.json"));
    let doc = FindingProposedIntakeV1 {
        schema_version: 1,
        kind: "finding_proposed",
        finding_id,
        session_id,
        claim_ids,
        worthiness_score,
        received_at_ms: received_at_ms(),
    };
    let bytes =
        serde_json::to_vec_pretty(&doc).map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
    atomic_write_json(&path, &bytes)?;
    Ok(path)
}

/// Persist a publication success observation (nanopub / venue outcome relay).
pub fn record_publication_succeeded(
    repo_root: &Path,
    manifest_id: &str,
    doi: Option<&str>,
    nanopub_uris: &[String],
) -> Result<PathBuf> {
    let root = mesh_intake_root(repo_root).join("publication-outcomes");
    let stem = sanitize_filename_component(manifest_id);
    let ts = received_at_ms();
    let path = root.join(format!("{ts}_{stem}.json"));
    let doc = PublicationSucceededIntakeV1 {
        schema_version: 1,
        kind: "publication_succeeded",
        manifest_id,
        received_at_ms: ts,
        doi,
        nanopub_uris,
    };
    let bytes =
        serde_json::to_vec_pretty(&doc).map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
    atomic_write_json(&path, &bytes)?;
    Ok(path)
}

/// Persist a publication failure observation.
pub fn record_publication_failed(
    repo_root: &Path,
    manifest_id: &str,
    error: &str,
) -> Result<PathBuf> {
    let root = mesh_intake_root(repo_root).join("publication-outcomes");
    let stem = sanitize_filename_component(manifest_id);
    let ts = received_at_ms();
    let path = root.join(format!("{ts}_{stem}_failed.json"));
    let doc = PublicationFailedIntakeV1 {
        schema_version: 1,
        kind: "publication_failed",
        manifest_id,
        received_at_ms: ts,
        error,
    };
    let bytes =
        serde_json::to_vec_pretty(&doc).map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
    atomic_write_json(&path, &bytes)?;
    Ok(path)
}

static INTAKE_VALIDATOR: LazyLock<vox_jsonschema_util::Validator> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/scientia/research-mesh-intake.v1.schema.json"
    )))
    .expect("intake schema JSON");
    vox_jsonschema_util::compile_validator(&schema, "research-mesh-intake.v1")
        .expect("compile intake schema")
});

/// Promoted ledger directory (`events.v1.jsonl`).
#[must_use]
pub fn mesh_promoted_dir(repo_root: &Path) -> PathBuf {
    vox_config::paths::repo_scientia_research_mesh_promoted_dir(repo_root)
}

/// Summary from [`consume_pending_intake`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConsumeSummary {
    pub promoted: usize,
    pub errors: Vec<String>,
}

fn pending_json_files(queue_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if !queue_dir.is_dir() {
        return Ok(out);
    }
    for entry in fs::read_dir(queue_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        if path.extension().and_then(|x| x.to_str()) == Some("json") {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

fn unique_processed_path(processed_dir: &Path, original: &Path) -> PathBuf {
    let Some(name) = original.file_name().and_then(|n| n.to_str()) else {
        return processed_dir.join("unknown.json");
    };
    let mut dest = processed_dir.join(name);
    if !dest.exists() {
        return dest;
    }
    let ts = received_at_ms();
    let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("json");
    dest = processed_dir.join(format!("{stem}_{ts}.{ext}"));
    let mut n = 0u32;
    while dest.exists() {
        n += 1;
        dest = processed_dir.join(format!("{stem}_{ts}_{n}.{ext}"));
    }
    dest
}

/// Validate pending intake JSON files, append each to `research-mesh-promoted/events.v1.jsonl`,
/// and move sources into a per-queue `processed/` directory.
pub fn consume_pending_intake(repo_root: &Path) -> Result<ConsumeSummary> {
    let intake_root = mesh_intake_root(repo_root);
    let promoted_root = mesh_promoted_dir(repo_root);
    fs::create_dir_all(&promoted_root)?;
    let ledger_path = promoted_root.join("events.v1.jsonl");

    let queues = [
        intake_root.join("finding-proposed"),
        intake_root.join("publication-outcomes"),
    ];

    let mut summary = ConsumeSummary::default();
    let mut ledger = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ledger_path)?;

    for queue in queues {
        let processed_dir = queue.join("processed");
        fs::create_dir_all(&processed_dir)?;
        for path in pending_json_files(&queue)? {
            let raw = match fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    summary
                        .errors
                        .push(format!("read {}: {e}", path.display()));
                    continue;
                }
            };
            let record: Value = match serde_json::from_str(&raw) {
                Ok(v) => v,
                Err(e) => {
                    summary
                        .errors
                        .push(format!("parse {}: {e}", path.display()));
                    continue;
                }
            };
            if let Err(e) = vox_jsonschema_util::validate(
                &record,
                &INTAKE_VALIDATOR,
                path.display(),
            ) {
                summary.errors.push(format!("{e:#}"));
                continue;
            }

            let rel = path
                .strip_prefix(&intake_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");

            let line = json!({
                "schema_version": 1,
                "promoted_at_ms": received_at_ms(),
                "source_relative_path": rel,
                "record": record,
            });
            let encoded = serde_json::to_string(&line)
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
            writeln!(ledger, "{encoded}").map_err(std::io::Error::other)?;

            let dest = unique_processed_path(&processed_dir, &path);
            if let Err(e) = fs::rename(&path, &dest) {
                summary.errors.push(format!(
                    "rename {} → {}: {e}",
                    path.display(),
                    dest.display()
                ));
                continue;
            }
            summary.promoted += 1;
        }
    }

    Ok(summary)
}

/// Background poll: runs [`consume_pending_intake`] on an interval (MCP / daemon hook).
pub fn spawn_research_mesh_intake_consumer(repo_root: PathBuf, interval: std::time::Duration) {
    tokio::spawn(async move {
        tracing::info!(
            target: "vox_publisher::research_mesh",
            repo_root = %repo_root.display(),
            interval_secs = interval.as_secs(),
            "research_mesh_intake_consumer_started"
        );
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match consume_pending_intake(&repo_root) {
                Ok(summary) => {
                    if !summary.errors.is_empty() {
                        tracing::warn!(
                            target: "vox_publisher::research_mesh",
                            errors = ?summary.errors,
                            "research_mesh_intake_consumer_validation_errors"
                        );
                    }
                    if summary.promoted > 0 {
                        tracing::info!(
                            target: "vox_publisher::research_mesh",
                            promoted = summary.promoted,
                            "research_mesh_intake_consumer_tick"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        target: "vox_publisher::research_mesh",
                        error = %e,
                        "research_mesh_intake_consumer_tick_failed"
                    );
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn finding_intake_validates_schema() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path =
            record_finding_candidate_proposed(dir.path(), "finding-42", "7", &[1, 2, 3], 0.82)
                .expect("write");
        assert!(path.exists());
        let raw = fs::read_to_string(&path).expect("read");
        let v: Value = serde_json::from_str(&raw).expect("json");
        let schema: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/scientia/research-mesh-intake.v1.schema.json"
        )))
        .expect("schema");
        let validator = jsonschema::validator_for(&schema).expect("compile");
        validator.validate(&v).expect("validates");
    }

    #[test]
    fn publication_failed_intake_validates_schema() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = record_publication_failed(dir.path(), "manifest-z", "upstream rejected")
            .expect("write");
        let raw = fs::read_to_string(&path).expect("read");
        let v: Value = serde_json::from_str(&raw).expect("json");
        let schema: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/scientia/research-mesh-intake.v1.schema.json"
        )))
        .expect("schema");
        let validator = jsonschema::validator_for(&schema).expect("compile");
        validator.validate(&v).expect("validates");
    }

    #[test]
    fn publication_succeeded_intake_validates_schema() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = record_publication_succeeded(
            dir.path(),
            "manifest-ok",
            Some("10.1234/example"),
            &["https://np.example/id".to_string()],
        )
        .expect("write");
        let raw = fs::read_to_string(&path).expect("read");
        let v: Value = serde_json::from_str(&raw).expect("json");
        let schema: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/scientia/research-mesh-intake.v1.schema.json"
        )))
        .expect("schema");
        let validator = jsonschema::validator_for(&schema).expect("compile");
        validator.validate(&v).expect("validates");
    }

    #[test]
    fn consume_promotes_intake_to_ledger_and_moves_processed() {
        let dir = tempfile::tempdir().expect("tempdir");
        record_finding_candidate_proposed(dir.path(), "f1", "s1", &[1], 0.9).expect("write");
        let summary = consume_pending_intake(dir.path()).expect("consume");
        assert_eq!(summary.promoted, 1);
        assert!(summary.errors.is_empty());

        let processed = mesh_intake_root(dir.path())
            .join("finding-proposed")
            .join("processed");
        assert_eq!(fs::read_dir(&processed).expect("rd").count(), 1);

        let ledger_path = mesh_promoted_dir(dir.path()).join("events.v1.jsonl");
        let raw = fs::read_to_string(&ledger_path).expect("ledger");
        let line = raw.lines().next().expect("line");
        let promoted: Value = serde_json::from_str(line).expect("promoted json");
        let promoted_schema: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/scientia/research-mesh-promoted-line.v1.schema.json"
        )))
        .expect("promoted schema");
        let pv = jsonschema::validator_for(&promoted_schema).expect("compile promoted");
        pv.validate(&promoted).expect("promoted line validates");

        vox_jsonschema_util::validate(&promoted["record"], &INTAKE_VALIDATOR, "record").expect("record");
    }
}
