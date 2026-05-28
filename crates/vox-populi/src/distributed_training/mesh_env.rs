//! Environment-backed mesh training toggles (replaces `vox-populi` `populi_train` stubs).

/// Configuration for a distributed GPU MENS training worker.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MeshTrainConfig {
    /// Total number of devices participating in the mesh run.
    pub world_size: usize,
    /// This worker's index `0..world_size-1`.
    pub rank: usize,
    /// Whether gradients sync after each accumulation step.
    pub gradient_reduce: bool,
}

#[must_use]
pub fn is_mesh_mode() -> bool {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshTrain)
        .expose()
        .map(|v: &str| v == "1")
        .unwrap_or(false)
}

#[must_use]
pub fn get_mesh_rank() -> usize {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshRank)
        .expose()
        .and_then(|v: &str| v.parse().ok())
        .unwrap_or(0)
}
