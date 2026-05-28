//! Retired-symbol proximity hints for queries (split-brain / migration drift).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::context::SearchRuntimeContext;
use crate::policy::SearchPolicy;

#[cfg(feature = "qdrant-vector")]
use crate::vector_qdrant::QdrantSemanticClient;

/// Cached `(schema_path, retired → canonical pairs)` so repeated scans avoid disk + JSON parse.
static SURFACE_CACHE: Mutex<Option<(PathBuf, Arc<Vec<SurfacePair>>)>> = Mutex::new(None);

#[derive(Clone)]
struct SurfacePair {
    retired: String,
    canonical: String,
}

fn levenshtein_chars(a: &[char], b: &[char]) -> usize {
    let n = a.len();
    let m = b.len();
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0usize; m + 1];
    for i in 1..=n {
        cur[0] = i;
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j - 1] + cost).min(prev[j] + 1).min(cur[j - 1] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[m]
}

/// Normalized similarity in `[0, 1]` from Levenshtein distance (1 = identical).
fn normalized_char_similarity(a: &str, b: &str) -> f64 {
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let d = levenshtein_chars(&ac, &bc);
    let max_len = ac.len().max(bc.len()).max(1);
    1.0_f64 - (d as f64 / max_len as f64)
}

fn load_surfaces(repo_root: &Path) -> Option<Arc<Vec<SurfacePair>>> {
    let schema_path = repo_root.join("contracts/proximity/retired-surfaces.v1.json");

    if let Ok(guard) = SURFACE_CACHE.lock()
        && let Some((ref cached_path, ref arc)) = *guard
        && cached_path == &schema_path
    {
        return Some(arc.clone());
    }

    let surfaces_json = vox_bounded_fs::read_utf8_path_capped(&schema_path).ok()?;
    let schema: serde_json::Value = serde_json::from_str(&surfaces_json).ok()?;
    let surfaces = schema
        .get("surfaces")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut pairs = Vec::new();
    for surface in surfaces {
        let retired = surface
            .get("retired_symbol")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let canonical = surface
            .get("canonical_replacement")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if retired.is_empty() || canonical.is_empty() {
            continue;
        }
        pairs.push(SurfacePair { retired, canonical });
    }

    let arc = Arc::new(pairs);
    if let Ok(mut guard) = SURFACE_CACHE.lock() {
        *guard = Some((schema_path, arc.clone()));
    }
    Some(arc)
}

/// Scans the project for symbol proximity (split-brain detection) based on the query.
pub async fn scan_symbol_proximity(
    ctx: &SearchRuntimeContext,
    query: &str,
    policy: &SearchPolicy,
    query_vector: Option<&[f32]>,
) -> Vec<String> {
    let mut hits = Vec::new();
    let mut qdrant_results: Vec<(String, f32, Option<String>)> = Vec::new();

    #[cfg(feature = "qdrant-vector")]
    {
        if let Some(url) = policy.qdrant_url.as_deref().filter(|u| !u.is_empty())
            && let Some(qv) = query_vector.filter(|v| !v.is_empty())
        {
            let client = QdrantSemanticClient::new(url, policy.qdrant_collection.as_str());
            let trace = ctx
                .trace_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());

            match client
                .search_vectors(qv, 5, policy.qdrant_vector_name.as_deref(), trace)
                .await
            {
                Ok(results) => qdrant_results = results,
                Err(e) => tracing::warn!(
                    error = %e,
                    "Qdrant vector search failed during symbol proximity scan; text-only matching continues",
                ),
            }
        }
    }

    let Some(surfaces) = load_surfaces(&ctx.repo_root) else {
        return hits;
    };

    for pair in surfaces.iter() {
        let retired = &pair.retired;
        let canonical = &pair.canonical;
        let query_lower = query.to_lowercase();
        let retired_lower = retired.to_lowercase();

        let mut max_ratio =
            if query_lower.contains(&retired_lower) || retired_lower.contains(&query_lower) {
                1.0
            } else {
                normalized_char_similarity(&query_lower, &retired_lower)
            };

        for (id, sc, _) in &qdrant_results {
            let id_l = id.to_lowercase();
            if id_l.contains(&retired_lower) || retired_lower.contains(&id_l) {
                max_ratio = (max_ratio + f64::from(*sc)) / 2.0;
            }
        }

        if max_ratio > 0.65 {
            hits.push(format!(
                "[proximity:{} score:{:.3}] `{}` shares semantic overlap with retired symbol `{}`. Canonical replacement is `{}`.",
                retired, max_ratio, query, retired, canonical
            ));
        }
    }

    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn normalized_similarity_identical_is_one() {
        assert!((normalized_char_similarity("foo", "foo") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn scan_loads_contract_from_repo_root() {
        let dir = tempdir().expect("tempdir");
        let contracts = dir.path().join("contracts/proximity");
        std::fs::create_dir_all(&contracts).expect("mkdir");
        std::fs::write(
            contracts.join("retired-surfaces.v1.json"),
            r#"{"surfaces":[{"retired_symbol":"legacy-split-parser","canonical_replacement":"vox-compiler"}]}"#,
        )
        .expect("write");

        let ctx = SearchRuntimeContext::new(
            dir.path().to_path_buf(),
            None,
            dir.path().to_path_buf(),
            dir.path().join("memory.md"),
        );

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let out = rt.block_on(scan_symbol_proximity(
            &ctx,
            "migrate from legacy-split-parser crate",
            &SearchPolicy::default(),
            None,
        ));
        assert!(
            out.iter()
                .any(|l| l.contains("legacy-split-parser") && l.contains("vox-compiler")),
            "{out:?}"
        );
    }
}
