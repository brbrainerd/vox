//! Conditional embedding sub-cluster: only buckets over the size threshold split.

use crate::bucket::Bucket;
use async_trait::async_trait;

/// Abstraction over embedding so tests can assert call counts and determinism.
#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, String>;
}

/// A sub-cluster of a bucket (or the whole bucket when not split).
#[derive(Debug, Clone)]
pub struct Cluster {
    pub key_suffix: String, // "" for unsplit, "-0"/"-1"/... for split
    pub bucket: Bucket,
}

/// Split oversized buckets into sub-clusters; pass small buckets through unchanged.
pub async fn maybe_split(
    buckets: Vec<Bucket>,
    max_bucket_size: usize,
    cluster_distance_threshold: f64,
    embedder: &dyn Embedder,
) -> Vec<Cluster> {
    let mut out = Vec::new();
    for b in buckets {
        if b.members.len() <= max_bucket_size {
            out.push(Cluster {
                key_suffix: String::new(),
                bucket: b,
            });
            continue;
        }
        // Embed each member's rationale; agglomerative cosine cluster.
        let mut vectors = Vec::with_capacity(b.members.len());
        let mut embed_failed = false;
        for m in &b.members {
            let text = m
                .row
                .finding
                .as_ref()
                .map(|f| f.rationale_one_line.clone())
                .unwrap_or_default();
            match embedder.embed(&text).await {
                Ok(v) => vectors.push(v),
                Err(_) => {
                    // Embedding failure → do not split this bucket.
                    embed_failed = true;
                    break;
                }
            }
        }
        if embed_failed {
            out.push(Cluster {
                key_suffix: String::new(),
                bucket: b,
            });
            continue;
        }
        let labels = agglomerative_cosine(&vectors, cluster_distance_threshold as f32);
        out.extend(split_by_labels(b, &labels));
    }
    out
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return 1.0;
    }
    1.0 - (dot / (na * nb))
}

/// Single-linkage agglomerative clustering: assign cluster ids by union-find
/// over pairs within `threshold` cosine distance.
fn agglomerative_cosine(vectors: &[Vec<f32>], threshold: f32) -> Vec<usize> {
    let n = vectors.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut Vec<usize>, x: usize) -> usize {
        if p[x] != x {
            let r = find(p, p[x]);
            p[x] = r;
        }
        p[x]
    }
    for i in 0..n {
        for j in (i + 1)..n {
            if cosine_distance(&vectors[i], &vectors[j]) <= threshold {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                if ri != rj {
                    parent[ri] = rj;
                }
            }
        }
    }
    // Normalize roots to dense 0..k labels.
    let mut label_of = std::collections::HashMap::new();
    let mut next = 0usize;
    (0..n)
        .map(|i| {
            let r = find(&mut parent, i);
            *label_of.entry(r).or_insert_with(|| {
                let l = next;
                next += 1;
                l
            })
        })
        .collect()
}

fn split_by_labels(b: Bucket, labels: &[usize]) -> Vec<Cluster> {
    let mut groups: std::collections::BTreeMap<usize, Vec<_>> = std::collections::BTreeMap::new();
    for (m, &l) in b.members.iter().zip(labels) {
        groups.entry(l).or_default().push(m.clone());
    }
    groups
        .into_iter()
        .map(|(l, members)| Cluster {
            key_suffix: format!("-{l}"),
            bucket: Bucket {
                key: b.key.clone(),
                members,
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bucket::BucketKey;
    use crate::load::LoadedFinding;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use vox_effort_audit::hybrid::MeasuredCost;
    use vox_effort_audit::judge::schema::{JudgeFinding, RemediationKind, WasteCategory};
    use vox_effort_audit::output::{FindingRow, JudgeMeta};
    use vox_effort_audit::shape::{CommitKind, ShapeFeatures};

    struct CountingMock {
        calls: AtomicUsize,
    }
    #[async_trait]
    impl Embedder for CountingMock {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(vec![1.0, 0.0, 0.0])
        }
    }

    /// Embedder that returns a caller-supplied vector per sequential call so
    /// tests can force distinct sub-clusters.
    struct ScriptedEmbedder {
        vectors: Vec<Vec<f32>>,
        next: AtomicUsize,
    }
    #[async_trait]
    impl Embedder for ScriptedEmbedder {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>, String> {
            let i = self.next.fetch_add(1, Ordering::SeqCst);
            Ok(self.vectors[i % self.vectors.len()].clone())
        }
    }

    fn member(sha: &str, rationale: &str) -> LoadedFinding {
        let row = FindingRow {
            schema_version: "1.0".into(),
            commit_sha: sha.into(),
            parent_sha: None,
            commit_ts: chrono::Utc::now(),
            author_email_sha256: "0".repeat(64),
            branch_hint: "main".into(),
            message_first_line: "test".into(),
            shape: ShapeFeatures {
                additions: 1,
                deletions: 0,
                files_changed: 1,
                file_extension_histogram: HashMap::new(),
                mechanical_sweep_score: 0.0,
                is_lockfile_only: false,
                is_generated_only: false,
                is_doc_only: false,
                commit_kind_from_message: CommitKind::Other,
            },
            cost: MeasuredCost::Unavailable,
            judge: JudgeMeta {
                model_id: "mock".into(),
                latency_ms: 0,
                judge_input_tokens: 0,
                judge_output_tokens: 0,
                outcome: "Judged".into(),
            },
            finding: Some(JudgeFinding {
                waste_score: 8,
                waste_category: WasteCategory::MechanicalSweep,
                suggested_remediation_kind: RemediationKind::ScriptAutomation,
                rationale_one_line: rationale.into(),
                evidence_pointers: vec!["crates/vox-config/src/x.rs:1".into()],
            }),
        };
        LoadedFinding { row }
    }

    fn bucket(n: usize) -> Bucket {
        Bucket {
            key: BucketKey {
                waste_category: "MechanicalSweep".into(),
                remediation_kind: "ScriptAutomation".into(),
                primary_crate: "vox-config".into(),
            },
            members: (0..n)
                .map(|i| member(&format!("sha{i}"), "same rationale"))
                .collect(),
        }
    }

    #[tokio::test]
    async fn small_bucket_does_not_embed() {
        // A bucket of 3 members (<= 20) must pass through untouched, 0 embed calls.
        let mock = CountingMock {
            calls: AtomicUsize::new(0),
        };
        let out = maybe_split(vec![bucket(3)], 20, 0.30, &mock).await;
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].key_suffix, "");
        assert_eq!(out[0].bucket.members.len(), 3);
        assert_eq!(mock.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn oversized_bucket_splits_on_distinct_vectors() {
        // 4 members > threshold 3: two on axis x, two on axis y → 2 sub-clusters.
        let mut b = bucket(4);
        b.members = (0..4).map(|i| member(&format!("sha{i}"), "r")).collect();
        let scripted = ScriptedEmbedder {
            vectors: vec![
                vec![1.0, 0.0, 0.0],
                vec![1.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0],
                vec![0.0, 1.0, 0.0],
            ],
            next: AtomicUsize::new(0),
        };
        let out = maybe_split(vec![b], 3, 0.30, &scripted).await;
        assert_eq!(
            out.len(),
            2,
            "two distinct embedding axes → two sub-clusters"
        );
        assert!(out.iter().all(|c| c.bucket.members.len() == 2));
        assert!(out.iter().all(|c| c.key_suffix.starts_with('-')));
    }

    #[tokio::test]
    async fn cosine_distance_basics() {
        assert!(cosine_distance(&[1.0, 0.0], &[1.0, 0.0]) < 1e-6);
        assert!((cosine_distance(&[1.0, 0.0], &[0.0, 1.0]) - 1.0).abs() < 1e-6);
    }
}
