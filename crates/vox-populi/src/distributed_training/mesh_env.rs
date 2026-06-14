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

#[cfg(test)]
mod semcov_wave1e_tests {
    #![allow(unused_imports)]
    use super::*;
    use serial_test::serial;

    /// is_mesh_mode() returns false when VOX_MESH_TRAIN is absent.
    #[test]
    #[serial]
    fn is_mesh_mode_absent_returns_false() {
        unsafe { std::env::remove_var("VOX_MESH_TRAIN") };
        assert!(!is_mesh_mode());
    }

    /// is_mesh_mode() returns true only when VOX_MESH_TRAIN == "1".
    #[test]
    #[serial]
    fn is_mesh_mode_set_to_one_returns_true() {
        unsafe { std::env::set_var("VOX_MESH_TRAIN", "1") };
        let result = is_mesh_mode();
        unsafe { std::env::remove_var("VOX_MESH_TRAIN") };
        assert!(result);
    }

    /// is_mesh_mode() returns false when value is not exactly "1".
    #[test]
    #[serial]
    fn is_mesh_mode_non_one_value_returns_false() {
        unsafe { std::env::set_var("VOX_MESH_TRAIN", "true") };
        let result = is_mesh_mode();
        unsafe { std::env::remove_var("VOX_MESH_TRAIN") };
        assert!(!result);
    }

    /// get_mesh_rank() returns 0 when VOX_MESH_RANK is absent.
    #[test]
    #[serial]
    fn get_mesh_rank_absent_returns_zero() {
        unsafe { std::env::remove_var("VOX_MESH_RANK") };
        assert_eq!(get_mesh_rank(), 0);
    }

    /// get_mesh_rank() parses a numeric rank correctly.
    #[test]
    #[serial]
    fn get_mesh_rank_parses_valid_rank() {
        unsafe { std::env::set_var("VOX_MESH_RANK", "3") };
        let r = get_mesh_rank();
        unsafe { std::env::remove_var("VOX_MESH_RANK") };
        assert_eq!(r, 3);
    }

    /// get_mesh_rank() falls back to 0 when the value is not a valid usize.
    #[test]
    #[serial]
    fn get_mesh_rank_invalid_value_returns_zero() {
        unsafe { std::env::set_var("VOX_MESH_RANK", "not_a_number") };
        let r = get_mesh_rank();
        unsafe { std::env::remove_var("VOX_MESH_RANK") };
        assert_eq!(r, 0);
    }
}
