//! Cross-manifest community drift and node/edge delta for two Graphify corpora.

/// A lightweight summary derived from a Graphify corpus manifest.
#[derive(Debug, Clone)]
pub struct ManifestSummary {
    pub node_count: u64,
    pub edge_count: u64,
    pub community_count: u64,
}

/// The delta between two manifest summaries (old → new).
#[derive(Debug, Clone)]
pub struct ManifestDiff {
    /// Change in node count (positive = growth, negative = shrinkage).
    pub node_delta: i64,
    /// Change in edge count.
    pub edge_delta: i64,
    /// Change in community count.
    pub community_delta: i64,
}

/// Compute the signed delta from `old` to `new`.
pub fn diff_manifests(old: &ManifestSummary, new: &ManifestSummary) -> ManifestDiff {
    ManifestDiff {
        node_delta: new.node_count as i64 - old.node_count as i64,
        edge_delta: new.edge_count as i64 - old.edge_count as i64,
        community_delta: new.community_count as i64 - old.community_count as i64,
    }
}
