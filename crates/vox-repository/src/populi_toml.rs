//! Optional **`[mesh]`** (canonical) or legacy **`[mens]`** section in workspace `Vox.toml`
//! (operator SSOT; env overrides apply in consumers).

use std::path::Path;

use serde::Deserialize;
use thiserror::Error;
use tracing::warn;

/// Mesh transport options (TLS, WireGuard hints).
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct VoxMeshTransport {
    /// PEM-encoded TLS certificate path. When `None`, server runs plain HTTP.
    pub tls_cert_path: Option<std::path::PathBuf>,
    /// PEM-encoded private key path. Required when `tls_cert_path` is set.
    pub tls_key_path: Option<std::path::PathBuf>,
    /// Optional minimum TLS version label: `"1.2"` or `"1.3"` (default `"1.3"`).
    pub tls_min_version: Option<String>,
    /// Documentation pointer: when set, operators are running behind a
    /// WireGuard sidecar (e.g., Tailscale Funnel). The server itself does
    /// nothing with this value — it is read by `vox doctor mesh`.
    pub wireguard_endpoint: Option<String>,
}

/// Parsed mesh coordination table from `Vox.toml` (`[mesh]` or legacy `[mens]`).
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct VoxMeshToml {
    /// HTTP control plane base URL (`GET /v1/populi/nodes`); maps to `OrchestratorConfig::populi_control_url`.
    #[serde(default)]
    pub control_url: Option<String>,
    /// Opaque cluster / tenancy id for join/heartbeat when the server enforces scope.
    #[serde(default)]
    pub scope_id: Option<String>,
    /// When true, merge `gpu_cuda` into default agent capabilities (same intent as `VOX_MESH_ADVERTISE_GPU`).
    #[serde(default)]
    pub advertise_gpu: Option<bool>,
    /// Extra capability labels (merged with env `VOX_MESH_LABELS`).
    #[serde(default)]
    pub labels: Option<Vec<String>>,
    /// Ollama-shaped local inference base URL (e.g. `http://127.0.0.1:11434` for Schola or Ollama.app).
    /// Maps to `OrchestratorConfig::populi_inference_base_url` (operators still set `POPULI_URL` unless tooling applies this).
    #[serde(default)]
    pub inference_base_url: Option<String>,
    /// Optional `[mesh.transport]` sub-table for TLS and WireGuard hints.
    #[serde(default)]
    pub transport: Option<VoxMeshTransport>,
}

/// Error reading or parsing `Vox.toml` for the mesh section only.
#[derive(Debug, Error)]
pub enum VoxMeshTomlError {
    /// Underlying I/O error.
    #[error("I/O reading Vox.toml: {0}")]
    Io(#[from] std::io::Error),
    /// TOML parse error.
    #[error("TOML parse error: {0}")]
    Parse(#[from] toml::de::Error),
    /// TOML serialize error (re-serializing a sub-table).
    #[error("TOML serialize error: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// English-first alias for [`VoxMeshToml`] (same type).
pub type MeshToml = VoxMeshToml;
/// English-first alias for [`VoxMeshTomlError`] (same type).
pub type MeshTomlError = VoxMeshTomlError;

fn parse_mesh_table(mesh_val: &toml::Value) -> Result<VoxMeshToml, VoxMeshTomlError> {
    let section_str = toml::to_string(mesh_val)?;
    Ok(toml::from_str(&section_str)?)
}

fn is_empty_mesh(m: &VoxMeshToml) -> bool {
    let labels_empty = match m.labels.as_ref() {
        None => true,
        Some(labels) => labels.is_empty(),
    };
    m.control_url.is_none()
        && m.scope_id.is_none()
        && m.advertise_gpu.is_none()
        && m.inference_base_url.is_none()
        && m.transport.is_none()
        && labels_empty
}

/// Read **`[mesh]`** or legacy **`[mens]`** from `path` (typically `.../Vox.toml`).
/// Returns `None` if the file or both sections are missing / empty.
pub fn read_vox_populi_toml(path: &Path) -> Result<Option<VoxMeshToml>, VoxMeshTomlError> {
    if !path.is_file() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)?;
    let table: toml::Table = toml::from_str(&content)?;

    let mesh_key = table.get("mesh");
    let mens_key = table.get("mens");

    let mesh_val = match (mesh_key, mens_key) {
        (Some(mesh), Some(_)) => {
            warn!(
                "Vox.toml {}: both [mesh] and [mens] are present; using [mesh] only (remove [mens])",
                path.display()
            );
            mesh
        }
        (Some(mesh), None) => mesh,
        (None, Some(mens)) => {
            warn!(
                "Vox.toml {}: [mens] is deprecated — rename to [mesh] (same keys)",
                path.display()
            );
            mens
        }
        (None, None) => return Ok(None),
    };

    let parsed = parse_mesh_table(mesh_val)?;
    if is_empty_mesh(&parsed) {
        return Ok(None);
    }
    Ok(Some(parsed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn reads_legacy_mens_section() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("Vox.toml");
        fs::write(
            &p,
            r#"
[mens]
control_url = "http://127.0.0.1:9999"
scope_id = "ci-scope"
advertise_gpu = true
labels = ["pool=a", "pool=b"]
"#,
        )
        .unwrap();
        let m = read_vox_populi_toml(&p).unwrap().expect("some mesh");
        assert_eq!(m.control_url.as_deref(), Some("http://127.0.0.1:9999"));
        assert_eq!(m.scope_id.as_deref(), Some("ci-scope"));
        assert_eq!(m.advertise_gpu, Some(true));
        assert_eq!(m.labels.as_ref().map(Vec::len), Some(2));
    }

    #[test]
    fn reads_canonical_mesh_section() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("Vox.toml");
        fs::write(
            &p,
            r#"
[mesh]
control_url = "http://127.0.0.1:10000"
scope_id = "mesh-scope"
inference_base_url = "http://127.0.0.1:11435"
"#,
        )
        .unwrap();
        let m = read_vox_populi_toml(&p).unwrap().expect("mesh");
        assert_eq!(m.control_url.as_deref(), Some("http://127.0.0.1:10000"));
        assert_eq!(m.scope_id.as_deref(), Some("mesh-scope"));
        assert_eq!(
            m.inference_base_url.as_deref(),
            Some("http://127.0.0.1:11435")
        );
    }

    #[test]
    fn prefers_mesh_when_both_mesh_and_mens_present() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("Vox.toml");
        fs::write(
            &p,
            r#"
[mesh]
control_url = "http://mesh-wins.example"
[mens]
control_url = "http://mens-ignored.example"
"#,
        )
        .unwrap();
        let m = read_vox_populi_toml(&p).unwrap().expect("mesh");
        assert_eq!(m.control_url.as_deref(), Some("http://mesh-wins.example"));
    }

    #[test]
    fn missing_file_returns_none() {
        let p = Path::new("/nonexistent/Vox.toml");
        assert!(read_vox_populi_toml(p).unwrap().is_none());
    }

    #[test]
    fn reads_mesh_transport_section() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("Vox.toml");
        fs::write(
            &p,
            r#"
[mesh]
control_url = "http://127.0.0.1:9999"
[mesh.transport]
tls_cert_path = "/etc/vox/cert.pem"
tls_key_path  = "/etc/vox/key.pem"
tls_min_version = "1.3"
"#,
        )
        .unwrap();
        let m = read_vox_populi_toml(&p).unwrap().expect("mesh");
        let t = m.transport.expect("transport");
        assert_eq!(
            t.tls_cert_path.unwrap(),
            std::path::PathBuf::from("/etc/vox/cert.pem")
        );
        assert_eq!(t.tls_min_version.as_deref(), Some("1.3"));
    }
}
