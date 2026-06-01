//! Training progress and GPU visibility via [`tracing`].
//!
//! Ported verbatim from `vox-populi/src/mens/tensor/train_log.rs` (SP3 sub-batch C).

use std::fmt::Display;

/// Format a scalar loss for **human-readable** progress logs.
#[must_use]
pub fn format_loss_for_log(loss: f64) -> String {
    if loss.is_nan() {
        return "nan".to_string();
    }
    if loss.is_infinite() {
        return if loss.is_sign_positive() {
            "inf".to_string()
        } else {
            "-inf".to_string()
        };
    }
    let ax = loss.abs();
    if ax == 0.0 {
        return "0".to_string();
    }
    if !(1e-3..1e3).contains(&ax) {
        format!("{loss:.4e}")
    } else {
        format!("{loss:.4}")
    }
}

pub fn info(msg: &str) {
    // Emit on tracing AND stderr. This crate is loaded as a dynamic cdylib whose
    // `tracing` dispatcher is NOT connected to the host's subscriber across the
    // dll boundary, so tracing-only events are silently dropped — leaving the
    // preflight/graph-build progress invisible during a run. eprintln on stderr
    // (unbuffered, boundary-safe) guarantees these diagnostics are visible live.
    tracing::info!(target: "vox_mens_train", "{}", msg);
    eprintln!("  [mens] {msg}");
}

pub fn warn(msg: &str) {
    tracing::warn!(target: "vox_mens_train", "{}", msg);
    eprintln!("  [mens][warn] {msg}");
}

pub fn _error(msg: impl Display) {
    tracing::error!(target: "vox_mens_train", "{}", msg);
}
