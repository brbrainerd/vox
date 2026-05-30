//! Top-level run: load → bucket → cluster → route(decide+verify) → emit.
//!
//! Composes the deterministic front half (load/filter/bucket) with the
//! conditional embedding sub-cluster and the per-cluster re-judge+verify
//! routing, then streams `recommendations.jsonl`, renders `recommendations.md`,
//! and writes any verified `.proposed` staging artifacts.
//!
//! Telemetry emission (`audit.route.*` events from `vox-telemetry`) is
//! intentionally deferred — the event types exist (D1) but wiring them here
//! would add a side-effect channel that complicates the deterministic e2e
//! smoke test. Follow-up: thread `vox_telemetry` emits at run.started /
//! cluster.decided / run.completed, mirroring S1's identical deferral.
//!
//! Concurrency: routing is sequential. Artifact writes + jsonl appends must be
//! serialized, and for S2's cluster counts (tens) sequential routing is
//! acceptable. A `Semaphore`-bounded `FuturesUnordered` (collect concurrently,
//! emit sequentially) is the documented upgrade path if a timing test shows
//! need — not done here to keep the budget accounting deterministic.

use crate::cluster::Embedder;
use crate::config::EffortRouteConfig;
use crate::emit::RecommendationRow;
use crate::route::{ModelVoxCapability, Router};
use std::path::Path;

/// Roll-up of a single `vox audit effort-route` run.
#[derive(Debug, Clone)]
pub struct RouteSummary {
    /// Stable id assigned to this run; prefixes every cluster id.
    pub run_id: String,
    /// Findings that survived load + filter.
    pub findings_loaded: usize,
    /// Deterministic buckets formed before sub-clustering.
    pub buckets: usize,
    /// Clusters routed (decide + verify).
    pub clusters_routed: usize,
    /// Clusters whose decision survived adversarial verification.
    pub verified: usize,
}

/// Run the full effort-route pipeline against an S1 `findings.jsonl`.
///
/// Writes `recommendations.jsonl`, `recommendations.md`, and verified
/// `.proposed` artifacts under `out_dir`. The `router` and `embedder` are
/// injected so tests can substitute deterministic mocks and the CLI can wire
/// the facade-backed LLM implementations.
pub async fn run(
    findings_path: &Path,
    out_dir: &Path,
    cfg: EffortRouteConfig,
    router: Box<dyn Router>,
    embedder: Box<dyn Embedder>,
    vox_capable: ModelVoxCapability,
) -> anyhow::Result<RouteSummary> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let loaded = crate::load::read(findings_path, cfg.min_waste_score)?;
    let findings_loaded = loaded.len();
    let buckets = crate::bucket::group(loaded);
    let bucket_count = buckets.len();
    let clusters =
        crate::cluster::maybe_split(buckets, cfg.max_bucket_size, embedder.as_ref()).await;

    std::fs::create_dir_all(out_dir)?;
    let mut writer = crate::emit::jsonl::JsonlWriter::create(&out_dir.join("recommendations.jsonl"))?;
    let mut rows: Vec<RecommendationRow> = Vec::new();
    let mut verified = 0usize;

    for (i, cluster) in clusters.iter().enumerate() {
        let cluster_id = format!("{run_id}-{i}");
        let decision = router.route(cluster, &cluster_id, vox_capable).await;
        if decision.verified {
            verified += 1;
        }
        crate::emit::artifacts::write_artifact(out_dir, &decision)?;
        let row = RecommendationRow::new(decision);
        writer.append(&row)?;
        rows.push(row);
    }

    std::fs::write(
        out_dir.join("recommendations.md"),
        crate::emit::markdown::render(&rows),
    )?;

    Ok(RouteSummary {
        run_id,
        findings_loaded,
        buckets: bucket_count,
        clusters_routed: clusters.len(),
        verified,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::MockRouter;
    use async_trait::async_trait;

    struct ZeroEmbedder;

    #[async_trait]
    impl Embedder for ZeroEmbedder {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>, String> {
            Ok(vec![0.0, 0.0, 0.0])
        }
    }

    fn fixture() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/findings.jsonl")
    }

    #[tokio::test]
    async fn run_loads_filters_and_emits() {
        let out = tempfile::tempdir().unwrap();
        let mut cfg = EffortRouteConfig::default();
        cfg.staging_dir = out.path().to_path_buf();

        let summary = run(
            &fixture(),
            out.path(),
            cfg,
            Box::new(MockRouter { confidence: 0.9 }),
            Box::new(ZeroEmbedder),
            ModelVoxCapability(false),
        )
        .await
        .unwrap();

        // The fixture has 2 surviving findings (null + low-score dropped).
        assert_eq!(summary.findings_loaded, 2);
        assert!(summary.clusters_routed >= 1);
        assert_eq!(summary.clusters_routed, summary.verified); // mock @0.9 verifies all
        assert!(out.path().join("recommendations.jsonl").exists());
        assert!(out.path().join("recommendations.md").exists());

        let lines = std::fs::read_to_string(out.path().join("recommendations.jsonl")).unwrap();
        assert_eq!(lines.lines().count(), summary.clusters_routed);
    }
}
