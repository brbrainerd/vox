//! Clavis-backed bearer resolution shared by [`crate::http_client`] and [`crate::transport::auth`].
//!
//! Keeps mesh/submitter/admin token trimming and empty-filter semantics in one place.

use std::sync::Arc;

use vox_secrets::SecretId;

#[must_use]
pub(crate) fn trimmed_nonempty_secret_arc(id: SecretId) -> Option<Arc<str>> {
    vox_secrets::resolve_secret(id)
        .expose()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| Arc::from(s.to_string().into_boxed_str()))
}

#[must_use]
pub(crate) fn trimmed_nonempty_secret_string(id: SecretId) -> Option<String> {
    trimmed_nonempty_secret_arc(id).map(|a| a.to_string())
}

/// Bearer for worker-plane routes (join / heartbeat / list): legacy mesh token only (`VOX_MESH_TOKEN`).
#[must_use]
pub(crate) fn mesh_worker_plane_bearer_string() -> Option<String> {
    trimmed_nonempty_secret_string(SecretId::VoxMeshToken)
}

/// Bearer for `POST /v1/populi/a2a/deliver`: first non-empty among mesh, submitter, admin tokens.
#[must_use]
pub(crate) fn deliver_bearer_string() -> Option<String> {
    trimmed_nonempty_secret_string(SecretId::VoxMeshToken)
        .or_else(|| trimmed_nonempty_secret_string(SecretId::VoxMeshSubmitterToken))
        .or_else(|| trimmed_nonempty_secret_string(SecretId::VoxMeshAdminToken))
}

#[cfg(test)]
// Rust 2024 made env mutation primitives `unsafe`. Tests in this module are
// gated by `serial_test::serial`, which is the SAFETY contract for every
// `unsafe` block touching `VOX_MESH_*`.
#[allow(unsafe_code)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn mesh_env() -> &'static str {
        SecretId::VoxMeshToken.spec().canonical_env
    }
    fn sub_env() -> &'static str {
        SecretId::VoxMeshSubmitterToken.spec().canonical_env
    }
    fn adm_env() -> &'static str {
        SecretId::VoxMeshAdminToken.spec().canonical_env
    }

    struct ClearMeshTokensOnDrop;
    impl Drop for ClearMeshTokensOnDrop {
        fn drop(&mut self) {
            // SAFETY: `serial_test::serial` ensures no concurrent env mutation in other tests.
            unsafe {
                for k in [mesh_env(), sub_env(), adm_env()] {
                    std::env::remove_var(k);
                }
            }
        }
    }

    #[test]
    #[serial]
    fn deliver_prefers_mesh_then_submitter_then_admin() {
        let _g = ClearMeshTokensOnDrop;
        // SAFETY: serialized test; temporary env overrides for Clavis resolution.
        unsafe {
            for k in [mesh_env(), sub_env(), adm_env()] {
                std::env::remove_var(k);
            }

            std::env::set_var(mesh_env(), "mesh-a");
            assert_eq!(deliver_bearer_string().as_deref(), Some("mesh-a"));

            std::env::remove_var(mesh_env());
            std::env::set_var(sub_env(), "sub-b");
            assert_eq!(deliver_bearer_string().as_deref(), Some("sub-b"));

            std::env::remove_var(sub_env());
            std::env::set_var(adm_env(), "adm-c");
            assert_eq!(deliver_bearer_string().as_deref(), Some("adm-c"));
        }
    }

    #[test]
    #[serial]
    fn mesh_worker_plane_is_mesh_token_only() {
        let _g = ClearMeshTokensOnDrop;
        // SAFETY: serialized test; temporary env overrides for Clavis resolution.
        unsafe {
            for k in [mesh_env(), sub_env(), adm_env()] {
                std::env::remove_var(k);
            }

            std::env::set_var(mesh_env(), "  tok  ");
            assert_eq!(mesh_worker_plane_bearer_string().as_deref(), Some("tok"));

            std::env::remove_var(mesh_env());
            std::env::set_var(sub_env(), "only-sub");
            assert!(
                mesh_worker_plane_bearer_string().is_none(),
                "submitter must not satisfy worker-plane bearer"
            );
        }
    }
}
