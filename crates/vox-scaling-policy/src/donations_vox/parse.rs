//! Parse a `donations.vox` source file into a `WorkerDonationPolicy`.

use std::path::Path;
use thiserror::Error;
use vox_mesh_types::donation_policy::{DonationSlot, WorkerDonationPolicy};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("Parse error in {path}: {message}")]
    Parse { path: String, message: String },
    #[error("Policy field missing: {field}")]
    MissingField { field: String },
}

/// Load and parse a `donations.vox` policy file.
///
/// The file uses Vox literal syntax for each field. This parser
/// extracts the key-value assignments from the file and maps them
/// to `WorkerDonationPolicy`.
pub fn load_policy(path: &Path) -> Result<WorkerDonationPolicy, ParseError> {
    let src = std::fs::read_to_string(path).map_err(|e| ParseError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    parse_source(&src, &path.display().to_string())
}

/// Parse a `donations.vox` source string into a `WorkerDonationPolicy`.
///
/// The canonical format for `donations.vox` is a set of Vox `let` bindings:
/// ```text
/// let nsfw_allowed = false
/// let max_job_duration_secs = 3600
/// let public_mesh_opt_in = false
/// let min_priority = 0
/// ```
/// Slot declarations use a `slot` keyword:
/// ```text
/// slot gpu { max_concurrent = 2, weight_pct = 60 }
/// slot cpu { max_concurrent = 4, weight_pct = 40 }
/// ```
///
/// Unknown keys are ignored for forward compatibility.
pub fn parse_source(src: &str, origin: &str) -> Result<WorkerDonationPolicy, ParseError> {
    // Minimal line-by-line parser for the canonical donations.vox format.
    // This is intentionally simpler than a full Vox parse — the policy file
    // has a constrained grammar. A future version may use vox-compiler's AST.
    let mut policy = WorkerDonationPolicy {
        slots: vec![],
        nsfw_allowed: false,
        max_job_duration_secs: 3600,
        public_mesh_opt_in: false,
        min_priority: 0,
        allowed_scopes: None,
        allowed_users: None,
        denied_users: None,
        allowed_mesh_networks: None,
        accept_sensitive_workloads: false,
        redundancy: None,
        accepts_inference_workloads: false,
        accepts_training_workloads: false,
        cuda_tier: 0,
        metal_tier: 0,
        vram_min_gb: 0,
        accepts_sensitive_training_data: false,
    };

    for (lineno, raw) in src.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        // `let <key> = <value>` assignments
        if let Some(rest) = line.strip_prefix("let ") {
            if let Some((key, val_str)) = rest.split_once('=') {
                let key = key.trim();
                let val = val_str.trim();
                match key {
                    "nsfw_allowed" => {
                        policy.nsfw_allowed = parse_bool(val).map_err(|e| ParseError::Parse {
                            path: origin.into(),
                            message: format!("line {}: nsfw_allowed: {e}", lineno + 1),
                        })?
                    }
                    "max_job_duration_secs" => {
                        policy.max_job_duration_secs =
                            val.parse().map_err(|_| ParseError::Parse {
                                path: origin.into(),
                                message: format!(
                                    "line {}: max_job_duration_secs must be u64",
                                    lineno + 1
                                ),
                            })?
                    }
                    "public_mesh_opt_in" => {
                        policy.public_mesh_opt_in =
                            parse_bool(val).map_err(|e| ParseError::Parse {
                                path: origin.into(),
                                message: format!("line {}: public_mesh_opt_in: {e}", lineno + 1),
                            })?
                    }
                    "min_priority" => {
                        policy.min_priority = val.parse().map_err(|_| ParseError::Parse {
                            path: origin.into(),
                            message: format!("line {}: min_priority must be u8", lineno + 1),
                        })?
                    }
                    "accept_sensitive_workloads" => {
                        policy.accept_sensitive_workloads =
                            parse_bool(val).map_err(|e| ParseError::Parse {
                                path: origin.into(),
                                message: format!(
                                    "line {}: accept_sensitive_workloads: {e}",
                                    lineno + 1
                                ),
                            })?
                    }
                    "accepts_inference_workloads" => {
                        policy.accepts_inference_workloads =
                            parse_bool(val).map_err(|e| ParseError::Parse {
                                path: origin.into(),
                                message: format!(
                                    "line {}: accepts_inference_workloads: {e}",
                                    lineno + 1
                                ),
                            })?
                    }
                    "accepts_training_workloads" => {
                        policy.accepts_training_workloads =
                            parse_bool(val).map_err(|e| ParseError::Parse {
                                path: origin.into(),
                                message: format!(
                                    "line {}: accepts_training_workloads: {e}",
                                    lineno + 1
                                ),
                            })?
                    }
                    "cuda_tier" => {
                        policy.cuda_tier = val.parse().map_err(|_| ParseError::Parse {
                            path: origin.into(),
                            message: format!("line {}: cuda_tier must be u8", lineno + 1),
                        })?
                    }
                    "metal_tier" => {
                        policy.metal_tier = val.parse().map_err(|_| ParseError::Parse {
                            path: origin.into(),
                            message: format!("line {}: metal_tier must be u8", lineno + 1),
                        })?
                    }
                    "vram_min_gb" => {
                        policy.vram_min_gb = val.parse().map_err(|_| ParseError::Parse {
                            path: origin.into(),
                            message: format!("line {}: vram_min_gb must be u32", lineno + 1),
                        })?
                    }
                    "accepts_sensitive_training_data" => {
                        policy.accepts_sensitive_training_data =
                            parse_bool(val).map_err(|e| ParseError::Parse {
                                path: origin.into(),
                                message: format!(
                                    "line {}: accepts_sensitive_training_data: {e}",
                                    lineno + 1
                                ),
                            })?
                    }
                    _ => {} // ignore unknown keys
                }
            }
        }
        // `slot <kind> { max_concurrent = N, weight_pct = M }` declarations
        else if let Some(rest) = line.strip_prefix("slot ")
            && let Some(brace_start) = rest.find('{')
        {
            let kind = rest[..brace_start].trim().to_string();
            let inner = rest[brace_start + 1..].trim_end_matches('}').trim();
            let mut max_concurrent: u8 = 1;
            let mut weight_pct: u8 = 50;
            for field in inner.split(',') {
                if let Some((k, v)) = field.trim().split_once('=') {
                    match k.trim() {
                        "max_concurrent" => {
                            max_concurrent = v.trim().parse().unwrap_or(1);
                        }
                        "weight_pct" => {
                            weight_pct = v.trim().parse().unwrap_or(50);
                        }
                        _ => {}
                    }
                }
            }
            use vox_mesh_types::task::TaskKind;
            let task_kind = TaskKind::from_str_loose(&kind);
            policy.slots.push(DonationSlot {
                task_kind,
                max_concurrent,
                weight_pct,
            });
        }
    }

    Ok(policy)
}

fn parse_bool(s: &str) -> Result<bool, String> {
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("expected 'true' or 'false', got '{other}'")),
    }
}
