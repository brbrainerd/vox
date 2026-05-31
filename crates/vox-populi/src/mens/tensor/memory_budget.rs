//! VRAM-aware training-config budgeting.
//!
//! Picks `(seq_len, batch_size, grad_accum)` that fit the available GPU VRAM for a
//! given model size, maximizing per-step token throughput while leaving a safety
//! margin so training does not OOM. Scales across GPUs: a 16 GB card gets a tight
//! config, an 80 GB card gets a generous one, all from the same formula.
//!
//! ## Why this exists
//!
//! QLoRA on a 4B model has a large *resident* footprint (base weights kept for the
//! forward pass + embeddings/LM-head + LoRA/optimizer state) that on a 16 GB card
//! leaves only a sliver for activations. A fixed preset (e.g. seq 512) silently
//! OOMs there while wasting capacity on an A100. The budget solves for the largest
//! activation footprint that fits and derives the config from it.
//!
//! ## Calibration
//!
//! Constants are calibrated from observed runs (Qwen3.5-4B QLoRA on a 16 GB RTX
//! 4080 Super sat at ~14 GB resident and OOMed near the ceiling). They are
//! deliberately conservative — the cost of under-using VRAM is slower training;
//! the cost of over-committing is a dead multi-hour run. `VOX_MENS_VRAM_SAFETY`
//! (0.5–0.98) tunes the aggressiveness.

/// Resident VRAM per billion parameters (GiB), covering base weights kept for the
/// forward pass plus a share of embedding/LM-head tensors. Calibrated so 4B ≈ 13 GiB.
const RESIDENT_GIB_PER_B_PARAMS: f64 = 3.25;

/// Fixed VRAM overhead (GiB): CUDA context, cuBLAS workspaces, allocator slack,
/// fragmentation headroom. Independent of model/sequence size.
const FIXED_OVERHEAD_GIB: f64 = 1.6;

/// Activation VRAM (GiB) per 1k tokens per unit of (params^0.5), per micro-batch.
/// Activation memory grows with sequence length and (sub-linearly) with model width.
/// Calibrated conservatively from the 4B/seq-512 peak.
const ACT_GIB_PER_KTOK_PER_SQRTB: f64 = 0.95;

/// Default fraction of total VRAM the plan is allowed to target.
const DEFAULT_SAFETY: f64 = 0.88;

/// Sequence-length search ladder (descending). The plan picks the largest that fits.
const SEQ_LADDER: &[usize] = &[1024, 768, 512, 384, 320, 256, 192, 160, 128];

/// Result of a budgeting pass.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetPlan {
    pub seq_len: usize,
    pub batch_size: usize,
    pub grad_accum: usize,
    /// True when even the smallest configuration is over budget — training may OOM;
    /// the caller should warn and consider a smaller model.
    pub over_budget: bool,
    /// Human-readable one-line rationale for logs.
    pub rationale: String,
}

/// Effective VRAM safety fraction, overridable via `VOX_MENS_VRAM_SAFETY`.
fn safety_fraction() -> f64 {
    std::env::var("VOX_MENS_VRAM_SAFETY")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|f| (0.5..=0.98).contains(f))
        .unwrap_or(DEFAULT_SAFETY)
}

/// Estimate activation VRAM (GiB) for one micro-batch at a given sequence length.
fn activation_gib(seq_len: usize, batch_size: usize, model_params_b: f64) -> f64 {
    let ktok = (seq_len as f64) * (batch_size as f64) / 1000.0;
    ktok * model_params_b.sqrt() * ACT_GIB_PER_KTOK_PER_SQRTB
}

/// Resident (sequence-independent) VRAM (GiB) for a model of `model_params_b` billions.
fn resident_gib(model_params_b: f64) -> f64 {
    model_params_b * RESIDENT_GIB_PER_B_PARAMS + FIXED_OVERHEAD_GIB
}

/// Target effective batch (batch_size × grad_accum) for stable QLoRA convergence.
/// Effective batch is kept roughly constant regardless of how the VRAM budget
/// splits it between micro-batch and accumulation.
const TARGET_EFFECTIVE_BATCH: usize = 8;

/// Compute a VRAM-fitting training config.
///
/// * `vram_gib` — total device VRAM in GiB (e.g. 16.0 for an RTX 4080 Super).
/// * `model_params_b` — model size in billions of parameters (e.g. 4.0).
///
/// Returns the largest `(seq_len, batch_size)` whose resident + activation estimate
/// fits `vram_gib × safety`, then sets `grad_accum` to hold the effective batch.
#[must_use]
pub fn plan(vram_gib: f64, model_params_b: f64) -> BudgetPlan {
    let safety = safety_fraction();
    let budget = vram_gib * safety;
    let resident = resident_gib(model_params_b);
    let activation_budget = budget - resident;

    // Not enough room for the model itself + any activations: floor the config and
    // flag it. The caller decides whether to proceed (it may still run with luck) or
    // recommend a smaller model.
    if activation_budget <= activation_gib(*SEQ_LADDER.last().unwrap(), 1, model_params_b) {
        let floor_seq = *SEQ_LADDER.last().unwrap();
        return BudgetPlan {
            seq_len: floor_seq,
            batch_size: 1,
            grad_accum: TARGET_EFFECTIVE_BATCH,
            over_budget: true,
            rationale: format!(
                "model resident ≈{resident:.1} GiB leaves only {activation_budget:.1} GiB of a \
                 {budget:.1} GiB budget for activations — below the floor for seq {floor_seq}. \
                 Using the smallest config; consider a smaller model or more VRAM."
            ),
        };
    }

    // Largest sequence length whose single-micro-batch activation fits the budget.
    let mut chosen_seq = *SEQ_LADDER.last().unwrap();
    for &seq in SEQ_LADDER {
        if activation_gib(seq, 1, model_params_b) <= activation_budget {
            chosen_seq = seq;
            break;
        }
    }

    // Grow micro-batch only if there is spare budget after fixing the sequence length;
    // otherwise keep batch 1 and use accumulation for the effective batch.
    let mut batch_size = 1usize;
    while batch_size < TARGET_EFFECTIVE_BATCH
        && activation_gib(chosen_seq, batch_size + 1, model_params_b) <= activation_budget
    {
        batch_size += 1;
    }

    let grad_accum = TARGET_EFFECTIVE_BATCH.div_ceil(batch_size).max(1);
    let used = resident + activation_gib(chosen_seq, batch_size, model_params_b);

    BudgetPlan {
        seq_len: chosen_seq,
        batch_size,
        grad_accum,
        over_budget: false,
        rationale: format!(
            "VRAM {vram_gib:.0} GiB × {safety:.2} = {budget:.1} GiB budget; model resident \
             ≈{resident:.1} GiB → seq {chosen_seq}, batch {batch_size}, grad_accum {grad_accum} \
             (≈{used:.1} GiB est. peak)."
        ),
    }
}

/// Parse a model's parameter count (in billions) from a model id / hint.
///
/// Recognizes patterns like `Qwen/Qwen3.5-4B`, `qwen2.5-0.8b`, `llama-9B`. Returns
/// `None` when no size token is present (caller falls back to a default).
#[must_use]
pub fn params_b_from_model_hint(hint: &str) -> Option<f64> {
    let lower = hint.to_ascii_lowercase();
    // Scan for a `<number>b` token (optionally decimal), preceded by a non-alnum or
    // a digit boundary so we don't match the `b` in arbitrary words.
    let bytes = lower.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'b' {
            // Walk backward over digits and a single dot.
            let mut j = i;
            let mut seen_digit = false;
            let mut seen_dot = false;
            while j > 0 {
                let c = bytes[j - 1];
                if c.is_ascii_digit() {
                    seen_digit = true;
                    j -= 1;
                } else if c == b'.' && !seen_dot {
                    seen_dot = true;
                    j -= 1;
                } else {
                    break;
                }
            }
            if seen_digit {
                if let Ok(v) = lower[j..i].parse::<f64>() {
                    if v > 0.0 && v < 2000.0 {
                        return Some(v);
                    }
                }
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_param_counts() {
        assert_eq!(params_b_from_model_hint("Qwen/Qwen3.5-4B"), Some(4.0));
        assert_eq!(params_b_from_model_hint("qwen2.5-0.8b"), Some(0.8));
        assert_eq!(params_b_from_model_hint("meta-llama/Llama-3-70B"), Some(70.0));
        assert_eq!(params_b_from_model_hint("some-model"), None);
    }

    #[test]
    fn larger_gpu_gets_larger_config() {
        let small = plan(16.0, 4.0);
        let big = plan(80.0, 4.0);
        // More VRAM → at least as long a sequence and at least as large an effective batch.
        assert!(big.seq_len >= small.seq_len);
        assert!(big.batch_size * big.grad_accum >= small.batch_size * small.grad_accum);
        assert!(!big.over_budget);
    }

    #[test]
    fn tiny_gpu_with_big_model_flags_over_budget() {
        // A 70B model cannot fit a 16 GB card.
        let p = plan(16.0, 70.0);
        assert!(p.over_budget);
        assert_eq!(p.batch_size, 1);
    }

    #[test]
    fn effective_batch_held_near_target() {
        let p = plan(80.0, 4.0);
        assert!(p.batch_size * p.grad_accum >= TARGET_EFFECTIVE_BATCH);
    }

    #[test]
    fn safety_env_override_tightens_budget() {
        // Lower safety → smaller-or-equal sequence length.
        let normal = plan(24.0, 4.0);
        // SAFETY: test-local env mutation; single-threaded test.
        unsafe { std::env::set_var("VOX_MENS_VRAM_SAFETY", "0.55") };
        let tight = plan(24.0, 4.0);
        unsafe { std::env::remove_var("VOX_MENS_VRAM_SAFETY") };
        assert!(tight.seq_len <= normal.seq_len);
    }
}
