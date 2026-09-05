#![cfg_attr(test, allow(unsafe_code))] // test-only std::env::set_var (unsafe on edition 2024)
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Hardware hints for a **task** requirement or an **agent** queue capability profile.
///
/// **CPU-first mens:** `cpu_cores`, `arch`, `hostname`, and `labels` describe the host; GPU / NPU
/// fields remain optional extensions. Deserialization fills missing fields from defaults so older
/// JSON/TOML remains valid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub struct TaskCapabilityHints {
    /// Task requires CUDA-capable execution; agent provides CUDA when true.
    #[serde(default)]
    pub gpu_cuda: bool,
    /// Task requires Metal; agent provides Metal when true.
    #[serde(default)]
    pub gpu_metal: bool,
    /// Agent advertises Vulkan-class GPU (typical Android / Linux).
    #[serde(default)]
    pub gpu_vulkan: bool,
    /// Agent advertises WebGPU-capable browser or host (soft; policy may disable WebGPU).
    #[serde(default)]
    pub gpu_webgpu: bool,
    /// Agent advertises an on-device NPU / neural accelerator.
    #[serde(default)]
    pub npu: bool,
    /// Agent/Task supports visual intelligence (VLM, OCR, layout audit).
    #[serde(default)]
    pub visus_eligible: bool,
    /// Agent/Task supports multi-modal payloads (image + text + audio).
    #[serde(default)]
    pub multi_modal: bool,
    /// Optional host class label (`server`, `desktop`, `mobile`, `browser`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_class: Option<String>,
    /// Minimum VRAM in MiB when GPU is required (soft hint for routing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_vram_mb: Option<u32>,
    /// Logical CPU count observed on the host (or operator override via config).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_cores: Option<u32>,
    /// Target architecture string (e.g. `x86_64`, `aarch64`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    /// Host name when known (mens / placement visibility).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    /// Optional scheduler labels (mens, region, pool, …).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// Task requires at least this many logical cores (soft routing penalty when unmet).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_cpu_cores: Option<u32>,
    /// Soft routing hint: deprioritize agents without any GPU capability (Mens-style training intent).
    #[serde(default)]
    pub prefer_gpu_compute: bool,
    /// Hardware/inference tier (e.g., "local").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing_tier: Option<String>,
    /// When true, Socrates low-confidence warnings cannot be bypassed by the agent.
    #[serde(default)]
    pub is_low_confidence_bypass_blocked: bool,
    /// When true, force a Socratic research pass even if confidence is moderate.
    #[serde(default)]
    pub force_socrates_research: bool,
}

/// Markers for tooling gates (Cargo, Node, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoCapabilities {
    /// `Vox.toml` present at repository root.
    pub vox_project: bool,
    /// Root `Cargo.toml` declares `[workspace]`.
    pub cargo_workspace: bool,
    /// Root `Cargo.toml` declares `[package]` (single crate or workspace member file mis-read guard).
    pub cargo_package: bool,
    /// `package.json` or `pnpm-workspace.yaml` at root.
    pub node_workspace: bool,
    /// `pyproject.toml` or `setup.py` at root.
    pub python_project: bool,
    /// `go.mod` at root.
    pub go_module: bool,
    /// Inside a Git work tree (`root` is under a `.git` ancestor or `git_root` matches).
    pub git: bool,
}

/// Probe capabilities for files under `root`.
pub fn probe_capabilities(root: &Path, in_git_work_tree: bool) -> RepoCapabilities {
    let cargo_toml = root.join("Cargo.toml");
    let mut cargo_workspace = false;
    let mut cargo_package = false;
    if cargo_toml.is_file()
        && let Ok(text) = std::fs::read_to_string(&cargo_toml)
        && let Ok(val) = toml::from_str::<toml::Value>(&text)
    {
        cargo_workspace = val.get("workspace").is_some();
        cargo_package = val.get("package").is_some();
    }
    RepoCapabilities {
        vox_project: root.join("Vox.toml").is_file(),
        cargo_workspace,
        cargo_package,
        node_workspace: root.join("package.json").is_file()
            || root.join("pnpm-workspace.yaml").is_file(),
        python_project: root.join("pyproject.toml").is_file() || root.join("setup.py").is_file(),
        go_module: root.join("go.mod").is_file(),
        git: in_git_work_tree,
    }
}

/// Snapshot of the current process host (best-effort, no extra crates required on the default build).
#[must_use]
pub fn probe_host_capabilities() -> TaskCapabilityHints {
    let mut h = TaskCapabilityHints {
        cpu_cores: std::thread::available_parallelism()
            .ok()
            .map(|n| n.get() as u32),
        arch: Some(std::env::consts::ARCH.to_string()),
        hostname: hostname_best_effort(),
        ..Default::default()
    };
    apply_mesh_capability_env(&mut h);
    h
}

/// Merge operator [`TaskCapabilityHints`] from config with a host [`probe_host_capabilities`] snapshot.
#[must_use]
pub fn merge_agent_capabilities(
    config: &TaskCapabilityHints,
    probed: TaskCapabilityHints,
) -> TaskCapabilityHints {
    let mut out = config.clone();
    if probed.cpu_cores.is_some() {
        out.cpu_cores = probed.cpu_cores;
    }
    if probed.arch.is_some() {
        out.arch = probed.arch;
    }
    if probed.hostname.is_some() {
        out.hostname = probed.hostname;
    }
    let mut seen: std::collections::HashSet<String> = out.labels.iter().cloned().collect();
    for lab in probed.labels {
        if seen.insert(lab.clone()) {
            out.labels.push(lab);
        }
    }
    if out.gpu_cuda {
        // config already wants CUDA
    } else if probed.gpu_cuda {
        out.gpu_cuda = true;
    }
    if out.gpu_metal {
        // config already wants Metal
    } else if probed.gpu_metal {
        out.gpu_metal = true;
    }
    if !out.gpu_vulkan && probed.gpu_vulkan {
        out.gpu_vulkan = true;
    }
    if !out.gpu_webgpu && probed.gpu_webgpu {
        out.gpu_webgpu = true;
    }
    if !out.npu && probed.npu {
        out.npu = true;
    }
    if !out.visus_eligible && probed.visus_eligible {
        out.visus_eligible = true;
    }
    if !out.multi_modal && probed.multi_modal {
        out.multi_modal = true;
    }
    if out.device_class.is_none() {
        out.device_class = probed.device_class.clone();
    }
    if out.min_vram_mb.is_none() {
        out.min_vram_mb = probed.min_vram_mb;
    }
    if out.min_cpu_cores.is_none() {
        out.min_cpu_cores = probed.min_cpu_cores;
    }
    if out.routing_tier.is_none() {
        out.routing_tier = probed.routing_tier;
    }
    out
}

fn hostname_best_effort() -> Option<String> {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .ok()
        .filter(|s| !s.is_empty())
}

fn mesh_env_truthy(name: &str) -> bool {
    std::env::var(name)
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false)
}

fn apply_mesh_capability_env(h: &mut TaskCapabilityHints) {
    if mesh_env_truthy("VOX_MESH_ADVERTISE_GPU") {
        h.gpu_cuda = true;
    }
    if mesh_env_truthy("VOX_MESH_ADVERTISE_VULKAN") {
        h.gpu_vulkan = true;
    }
    if mesh_env_truthy("VOX_MESH_ADVERTISE_WEBGPU") {
        h.gpu_webgpu = true;
    }
    if mesh_env_truthy("VOX_MESH_ADVERTISE_NPU") {
        h.npu = true;
    }
    if let Ok(s) = std::env::var("VOX_MESH_DEVICE_CLASS") {
        let t = s.trim();
        if !t.is_empty() {
            h.device_class = Some(t.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_capability_hints_deserialize_omitted_fields() {
        let j = r#"{"gpu_cuda":true}"#;
        let h: TaskCapabilityHints = serde_json::from_str(j).unwrap();
        assert!(h.gpu_cuda);
        assert!(!h.gpu_metal);
        assert!(!h.gpu_vulkan);
        assert!(!h.gpu_webgpu);
        assert!(!h.npu);
        assert!(!h.visus_eligible);
        assert!(!h.multi_modal);
        assert!(h.device_class.is_none());
        assert!(h.cpu_cores.is_none());
        assert!(h.labels.is_empty());
    }

    #[test]
    fn merge_prefers_probed_cpu_and_keeps_config_gpu() {
        let cfg = TaskCapabilityHints {
            gpu_cuda: true,
            labels: vec!["pool=a".into()],
            ..Default::default()
        };
        let p = TaskCapabilityHints {
            cpu_cores: Some(8),
            arch: Some("aarch64".into()),
            labels: vec!["pool=b".into()],
            ..Default::default()
        };
        let m = merge_agent_capabilities(&cfg, p);
        assert_eq!(m.cpu_cores, Some(8));
        assert_eq!(m.arch.as_deref(), Some("aarch64"));
        assert!(m.gpu_cuda);
        assert!(m.labels.contains(&"pool=a".into()));
        assert!(m.labels.contains(&"pool=b".into()));
    }
}

#[cfg(test)]
mod semcov_wave2_tests {
    // These tests mutate PROCESS-GLOBAL env vars. `cargo test` runs them on a
    // thread pool, not single-threaded as an earlier comment here claimed — one
    // test removing VOX_MESH_ADVERTISE_GPU raced a sibling setting it, and
    // `apply_mesh_env_does_not_set_flag_when_env_absent` failed intermittently
    // (reproduced: 2 of 5 parallel runs on an 18-core host; always green under
    // --test-threads=1). Every test that touches the environment must hold
    // ENV_LOCK for the whole set/act/assert sequence.
    #![allow(unused_imports, unsafe_code)]
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn env_lock() -> MutexGuard<'static, ()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        // A panicking test poisons the lock; the guarded state is the process
        // environment, which each test sets before reading, so recovering is safe
        // and avoids one failure cascading into three misleading ones.
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn apply_mesh_env_sets_gpu_cuda_when_env_is_one() {
        let _guard = env_lock();
        unsafe { std::env::set_var("VOX_MESH_ADVERTISE_GPU", "1") };
        let mut h = TaskCapabilityHints::default();
        apply_mesh_capability_env(&mut h);
        unsafe { std::env::remove_var("VOX_MESH_ADVERTISE_GPU") };
        assert!(h.gpu_cuda);
    }

    #[test]
    fn apply_mesh_env_sets_gpu_cuda_when_env_is_true_mixed_case() {
        let _guard = env_lock();
        unsafe { std::env::set_var("VOX_MESH_ADVERTISE_GPU", "True") };
        let mut h = TaskCapabilityHints::default();
        apply_mesh_capability_env(&mut h);
        unsafe { std::env::remove_var("VOX_MESH_ADVERTISE_GPU") };
        assert!(h.gpu_cuda);
    }

    #[test]
    fn apply_mesh_env_does_not_set_flag_when_env_absent() {
        let _guard = env_lock();
        unsafe { std::env::remove_var("VOX_MESH_ADVERTISE_GPU") };
        let mut h = TaskCapabilityHints::default();
        apply_mesh_capability_env(&mut h);
        assert!(!h.gpu_cuda);
    }

    #[test]
    fn apply_mesh_env_sets_device_class_from_env() {
        let _guard = env_lock();
        unsafe { std::env::set_var("VOX_MESH_DEVICE_CLASS", "server") };
        let mut h = TaskCapabilityHints::default();
        apply_mesh_capability_env(&mut h);
        unsafe { std::env::remove_var("VOX_MESH_DEVICE_CLASS") };
        assert_eq!(h.device_class.as_deref(), Some("server"));
    }

    #[test]
    fn probe_host_capabilities_returns_arch() {
        let h = probe_host_capabilities();
        // arch is always populated
        assert!(h.arch.is_some());
        let arch = h.arch.unwrap();
        assert!(!arch.is_empty());
    }

    #[test]
    fn probe_host_capabilities_cpu_cores_positive() {
        let h = probe_host_capabilities();
        if let Some(cores) = h.cpu_cores {
            assert!(cores >= 1);
        }
    }
}
