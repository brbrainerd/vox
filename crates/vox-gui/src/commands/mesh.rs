//! Tauri commands for the Settings → "Mesh & peers" surface.
//!
//! READ side: the live peer/node list is fetched by the frontend through the
//! existing `vox_mesh_nodes` MCP tool (control-plane-preferred, local-registry
//! fallback) — the same source the Mesh surface uses, so the data is coherent
//! with the one shared orchestrator. We do NOT duplicate that here.
//!
//! WRITE side (peer trust): trust is a REAL, local concept backed by
//! [`vox_identity::TrustedNodeRegistry`] (`~/.vox/trusted_nodes.json`), the same
//! store the `vox auth trust` / `vox auth untrust` CLI commands write. A peer is
//! "trusted" when its node_id is present in that registry. These commands expose
//! list / trust / untrust so the GUI "manage" button performs a real mutation.

use serde::Serialize;
use tauri::command;
use vox_identity::TrustedNodeRegistry;

/// One locally-trusted peer. Mirrors [`vox_identity::TrustedNode`]; all fields
/// are public (the trust store holds public keys only, never secrets).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustedNodeDto {
    pub node_id: String,
    pub pubkey_hex: String,
    pub label: Option<String>,
    pub added_at: String,
}

impl From<vox_identity::TrustedNode> for TrustedNodeDto {
    fn from(n: vox_identity::TrustedNode) -> Self {
        Self {
            node_id: n.node_id,
            pubkey_hex: n.pubkey_hex,
            label: n.label,
            added_at: n.added_at,
        }
    }
}

/// List every locally-trusted peer (the trust binding for mesh nodes).
#[command]
pub fn list_trusted_nodes() -> Result<Vec<TrustedNodeDto>, String> {
    TrustedNodeRegistry::new()
        .list()
        .map(|v| v.into_iter().map(TrustedNodeDto::from).collect())
        .map_err(|e| e.to_string())
}

/// Trust a mesh peer by adding its node_id + public key to the local registry.
///
/// `node_id` is the id surfaced by `vox_mesh_nodes`. `pubkey_hex` is the node's
/// advertised Ed25519 public key (hex). When the node advertises a base64 key
/// instead, the caller should hex-encode it first; if the key is unavailable the
/// node_id is still recorded so the binding is honest about what it knows.
#[command]
pub fn trust_mesh_node(
    node_id: String,
    pubkey_hex: String,
    label: Option<String>,
) -> Result<bool, String> {
    if node_id.trim().is_empty() {
        return Err("node_id must not be empty".to_string());
    }
    TrustedNodeRegistry::new()
        .add(node_id, pubkey_hex, label)
        .map(|_| true)
        .map_err(|e| e.to_string())
}

/// Untrust a mesh peer: remove its node_id from the local registry. Returns
/// whether a binding was actually removed.
#[command]
pub fn untrust_mesh_node(node_id: String) -> Result<bool, String> {
    if node_id.trim().is_empty() {
        return Err("node_id must not be empty".to_string());
    }
    TrustedNodeRegistry::new()
        .remove(&node_id)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dto_maps_all_fields() {
        let node = vox_identity::TrustedNode {
            node_id: "abc".into(),
            pubkey_hex: "ff".into(),
            label: Some("forge".into()),
            added_at: "2026-01-01T00:00:00Z".into(),
        };
        let dto = TrustedNodeDto::from(node);
        assert_eq!(dto.node_id, "abc");
        assert_eq!(dto.pubkey_hex, "ff");
        assert_eq!(dto.label.as_deref(), Some("forge"));
        assert_eq!(dto.added_at, "2026-01-01T00:00:00Z");
    }

    #[test]
    fn trust_rejects_empty_node_id() {
        assert!(trust_mesh_node("  ".into(), "ff".into(), None).is_err());
    }

    #[test]
    fn untrust_rejects_empty_node_id() {
        assert!(untrust_mesh_node("".into()).is_err());
    }
}
