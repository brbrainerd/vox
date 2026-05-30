//! e2e: run the pipeline against a fixture findings.jsonl with MockRouter + a
//! zero-vector embedder (small buckets never embed, but the signature requires
//! an `Embedder`).

use async_trait::async_trait;
use std::path::PathBuf;
use vox_effort_route::cluster::Embedder;
use vox_effort_route::config::EffortRouteConfig;
use vox_effort_route::route::{MockRouter, ModelVoxCapability};

/// Deterministic embedder for tests; returns a constant vector. Never actually
/// called for the small fixture buckets, but satisfies the `run` signature.
struct ZeroEmbedder;

#[async_trait]
impl Embedder for ZeroEmbedder {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, String> {
        Ok(vec![0.0, 0.0, 0.0])
    }
}

#[tokio::test]
async fn smoke_run_produces_outputs() {
    let findings = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/findings.jsonl");
    let out = tempfile::tempdir().unwrap();
    let cfg = EffortRouteConfig {
        staging_dir: out.path().to_path_buf(),
        ..Default::default()
    };

    let summary = vox_effort_route::run(
        &findings,
        out.path(),
        cfg,
        Box::new(MockRouter { confidence: 0.9 }),
        Box::new(ZeroEmbedder),
        ModelVoxCapability(false),
    )
    .await
    .unwrap();

    assert!(out.path().join("recommendations.jsonl").exists());
    assert!(out.path().join("recommendations.md").exists());
    // 2 surviving findings → at least 1 cluster, at least 1 verified recommendation.
    assert!(summary.clusters_routed >= 1);
    assert!(summary.verified >= 1);

    // recommendations.md must not leak author identity.
    let md = std::fs::read_to_string(out.path().join("recommendations.md")).unwrap();
    assert!(!md.contains('@'), "report leaked an email: {md}");
}
