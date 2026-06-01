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
/// forward pass plus a share of embedding/LM-head tensors.
///
/// Calibrated from hardware: a Qwen3.5-4B QLoRA run OOMed on a 16 GiB RTX 4080
/// Super even at seq 128, so resident(4B) must exceed ~15.5 GiB. With the fixed
/// overhead below, 3.5 GiB/B gives resident(4B) ≈ 15.6 GiB (rejected on 16 GiB,
/// fits 24 GiB) and resident(2B) ≈ 8.6 GiB (comfortable on 16 GiB).
const RESIDENT_GIB_PER_B_PARAMS: f64 = 3.5;

/// Fixed VRAM overhead (GiB): CUDA context, cuBLAS workspaces, allocator slack,
/// fragmentation headroom. Independent of model/sequence size.
const FIXED_OVERHEAD_GIB: f64 = 1.6;

/// Activation VRAM (GiB) per 1k tokens per unit of (params^0.5), per micro-batch.
/// Activation memory grows with sequence length and (sub-linearly) with model width.
///
/// Calibrated HEAVY (≈6.5) to the measured bf16 reality: activations stay F32 and are
/// retained across all layers for backward, so for 3B@seq512 they account for ~6.4 GiB
/// of the ~15.8 GiB peak. At this coefficient the budget keeps 3B at seq ≈ 384 on a
/// 16 GiB card (~14 GiB peak, a real ~2 GiB margin) rather than seq 512/1024 which run
/// at/over the edge. This is the dominant lever — sequence length, not base size.
const ACT_GIB_PER_KTOK_PER_SQRTB: f64 = 6.5;

/// Default fraction of total VRAM the plan is allowed to target.
const DEFAULT_SAFETY: f64 = 0.88;

/// Sequence-length search ladder (descending). The plan picks the largest that fits.
/// Extends to 2048 so large-VRAM cards (A100/H100) get longer contexts, down to 128
/// for the tightest configs.
const SEQ_LADDER: &[usize] = &[2048, 1536, 1024, 768, 512, 384, 320, 256, 192, 160, 128];

/// Qwen3.5 base-model ladder (largest → smallest): (parameter count in billions,
/// Hugging Face repo id). Used to auto-retreat to the largest variant that fits the
/// available VRAM. Mirrors the ids referenced across the codebase + `DEFAULT_MODEL_ID`.
pub const QWEN35_LADDER: &[(f64, &str)] = &[
    (9.0, "Qwen/Qwen3.5-9B"),
    (4.0, "Qwen/Qwen3.5-4B"),
    (2.0, "Qwen/Qwen3.5-2B"),
    (0.8, "Qwen/Qwen3.5-0.8B"),
];

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

/// Resident (sequence-independent) VRAM (GiB) for a model of `model_params_b`
/// billions, at a given per-B-param footprint. Different model families have
/// different footprints (Qwen3.5's 248k vocab + MoE/MTP overhead is heavier than
/// plain dense Qwen2 with a 151k vocab).
fn resident_gib_at(model_params_b: f64, resident_per_b: f64) -> f64 {
    model_params_b * resident_per_b + FIXED_OVERHEAD_GIB
}

/// Sequence-independent resident footprint for dense Qwen2 / Qwen2.5-Coder under
/// the bf16 base-matmul change (qlora-rs dequantizes the base weight to bf16).
///
/// Calibrated to MEASURED bf16 training peaks on a 16 GiB RTX 4080: 3B@seq512
/// plateaued at ~15.8 GiB (stable, no OOM). The base/embedding/LoRA/optimizer
/// (seq-independent) part is ≈9.4 GiB for 3B → R ≈ 2.6 GiB/B + the fixed overhead.
/// The F32 *activations* (kept F32 for stability) are the heavy swing term — see
/// ACT_GIB_PER_KTOK_PER_SQRTB — so the budget trades sequence length, not base size.
const QWEN2_RESIDENT_GIB_PER_B: f64 = 2.6;

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
    plan_with_resident(vram_gib, model_params_b, RESIDENT_GIB_PER_B_PARAMS)
}

/// As [`plan`], but with an explicit resident-footprint-per-billion-params so
/// different model families (Qwen3.5 vs dense Qwen2) get accurate budgets.
#[must_use]
pub fn plan_with_resident(vram_gib: f64, model_params_b: f64, resident_per_b: f64) -> BudgetPlan {
    let safety = safety_fraction();
    let budget = vram_gib * safety;
    let resident = resident_gib_at(model_params_b, resident_per_b);
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

/// A model + config plan: which Qwen3.5 variant to train and how to size it.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelPlan {
    /// Hugging Face repo id to train (may differ from the request if we retreated).
    pub model_id: String,
    pub params_b: f64,
    pub seq_len: usize,
    pub batch_size: usize,
    pub grad_accum: usize,
    /// `Some(requested_b)` when we retreated to a smaller model than requested.
    pub retreated_from_b: Option<f64>,
    /// True when even the smallest variant is over budget (very small card).
    pub over_budget: bool,
    pub rationale: String,
}

/// True when a model id belongs to the Qwen3.5 family this ladder manages.
#[must_use]
pub fn is_qwen35(model_id: &str) -> bool {
    let l = model_id.to_ascii_lowercase();
    l.contains("qwen3.5") || l.contains("qwen3-5") || l.contains("qwen35")
}

/// Pick the largest Qwen3.5 variant (no larger than `max_params_b`) that fits
/// `vram_gib`, and size its config. Retreats down the ladder (4B → 2B → 0.8B) when
/// the requested size does not fit, and scales the chosen model's sequence/batch up
/// on roomier cards. If nothing fits, returns the smallest variant flagged
/// `over_budget` so the caller can warn.
///
/// `max_params_b` caps the search at the requested size — we never auto-upgrade
/// past what the operator asked for (the default request is 4B via `DEFAULT_MODEL_ID`).
#[must_use]
pub fn plan_qwen35(vram_gib: f64, max_params_b: f64) -> ModelPlan {
    let mut smallest_tried: Option<ModelPlan> = None;

    for &(params, id) in QWEN35_LADDER {
        // Skip variants larger than the requested cap (no surprise upgrades).
        if params > max_params_b + 1e-9 {
            continue;
        }
        let p = plan(vram_gib, params);
        let retreated = (params - max_params_b).abs() > 1e-9;
        let retreated_from_b = retreated.then_some(max_params_b);
        let rationale = if retreated {
            format!(
                "requested ≈{max_params_b:.1}B does not fit {vram_gib:.0} GiB; retreated to \
                 {id} — {}",
                p.rationale
            )
        } else {
            format!("{id} — {}", p.rationale)
        };
        let mp = ModelPlan {
            model_id: id.to_string(),
            params_b: params,
            seq_len: p.seq_len,
            batch_size: p.batch_size,
            grad_accum: p.grad_accum,
            retreated_from_b,
            over_budget: p.over_budget,
            rationale,
        };
        if !p.over_budget {
            return mp; // largest variant (descending walk) that fits
        }
        smallest_tried = Some(mp);
    }

    // Nothing fit — return the smallest variant we tried, flagged over budget.
    smallest_tried.unwrap_or_else(|| {
        let (params, id) = *QWEN35_LADDER.last().unwrap();
        let p = plan(vram_gib, params);
        ModelPlan {
            model_id: id.to_string(),
            params_b: params,
            seq_len: p.seq_len,
            batch_size: p.batch_size,
            grad_accum: p.grad_accum,
            retreated_from_b: Some(max_params_b),
            over_budget: true,
            rationale: format!("no Qwen3.5 variant fits {vram_gib:.0} GiB; {}", p.rationale),
        }
    })
}

/// Qwen2.5-Coder ladder (largest → smallest): (parameter count in billions, HF repo id).
/// Plain dense `qwen2` coders — the path the candle plugin reliably trains (no MoE,
/// no MTP, no vision tower, no mRoPE). Verified available on the Qwen HF org.
pub const QWEN25CODER_LADDER: &[(f64, &str)] = &[
    (32.0, "Qwen/Qwen2.5-Coder-32B-Instruct"),
    (14.0, "Qwen/Qwen2.5-Coder-14B-Instruct"),
    (7.0, "Qwen/Qwen2.5-Coder-7B-Instruct"),
    (3.0, "Qwen/Qwen2.5-Coder-3B-Instruct"),
    (1.5, "Qwen/Qwen2.5-Coder-1.5B-Instruct"),
    (0.5, "Qwen/Qwen2.5-Coder-0.5B-Instruct"),
];

/// True when a model id is a Qwen2.5-Coder (the coding-focused dense family).
#[must_use]
pub fn is_qwen25coder(model_id: &str) -> bool {
    let l = model_id.to_ascii_lowercase();
    l.contains("qwen2.5-coder") || l.contains("qwen2_5-coder") || l.contains("qwen25-coder")
}

/// Pick the largest Qwen2.5-Coder variant (≤ `max_params_b`) that fits `vram_gib`,
/// sized with the lighter dense-Qwen2 resident footprint. Same retreat/scale
/// semantics as [`plan_qwen35`] but for the coding family.
#[must_use]
pub fn plan_qwen25coder(vram_gib: f64, max_params_b: f64) -> ModelPlan {
    let mut smallest_tried: Option<ModelPlan> = None;
    for &(params, id) in QWEN25CODER_LADDER {
        if params > max_params_b + 1e-9 {
            continue;
        }
        let p = plan_with_resident(vram_gib, params, QWEN2_RESIDENT_GIB_PER_B);
        let retreated = (params - max_params_b).abs() > 1e-9;
        let rationale = if retreated {
            format!("requested ≈{max_params_b:.1}B does not fit {vram_gib:.0} GiB; retreated to {id} — {}", p.rationale)
        } else {
            format!("{id} — {}", p.rationale)
        };
        let mp = ModelPlan {
            model_id: id.to_string(),
            params_b: params,
            seq_len: p.seq_len,
            batch_size: p.batch_size,
            grad_accum: p.grad_accum,
            retreated_from_b: retreated.then_some(max_params_b),
            over_budget: p.over_budget,
            rationale,
        };
        if !p.over_budget {
            return mp;
        }
        smallest_tried = Some(mp);
    }
    smallest_tried.unwrap_or_else(|| {
        let (params, id) = *QWEN25CODER_LADDER.last().unwrap();
        let p = plan_with_resident(vram_gib, params, QWEN2_RESIDENT_GIB_PER_B);
        ModelPlan {
            model_id: id.to_string(),
            params_b: params,
            seq_len: p.seq_len,
            batch_size: p.batch_size,
            grad_accum: p.grad_accum,
            retreated_from_b: Some(max_params_b),
            over_budget: true,
            rationale: format!("no Qwen2.5-Coder variant fits {vram_gib:.0} GiB; {}", p.rationale),
        }
    })
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
    fn ladder_retreats_4b_to_2b_on_16gb() {
        // 16 GiB cannot fit 4B → retreat to the largest variant that fits (2B),
        // which should get a comfortable (not floored) sequence length.
        let p = plan_qwen35(16.0, 4.0);
        assert_eq!(p.model_id, "Qwen/Qwen3.5-2B");
        assert_eq!(p.retreated_from_b, Some(4.0));
        assert!(!p.over_budget);
        assert!(p.seq_len >= 512, "2B on 16 GiB should afford a long sequence");
    }

    #[test]
    fn ladder_keeps_4b_on_24gb() {
        let p = plan_qwen35(24.0, 4.0);
        assert_eq!(p.model_id, "Qwen/Qwen3.5-4B");
        assert_eq!(p.retreated_from_b, None);
        assert!(!p.over_budget);
    }

    #[test]
    fn ladder_scales_up_for_big_cards() {
        // 80 GiB / 4B → keep 4B with a long sequence and real batch.
        let p = plan_qwen35(80.0, 4.0);
        assert_eq!(p.model_id, "Qwen/Qwen3.5-4B");
        assert!(p.seq_len >= 1024);
        assert!(p.batch_size >= 2);
    }

    #[test]
    fn ladder_floors_to_smallest_on_tiny_card() {
        // 4 GiB cannot fit any variant comfortably → smallest, flagged over budget.
        let p = plan_qwen35(4.0, 4.0);
        assert_eq!(p.model_id, "Qwen/Qwen3.5-0.8B");
        assert!(p.over_budget);
    }

    #[test]
    fn ladder_does_not_upgrade_past_request() {
        // Requesting 2B on a huge card stays 2B (no surprise upgrade to 4B/9B).
        let p = plan_qwen35(80.0, 2.0);
        assert_eq!(p.model_id, "Qwen/Qwen3.5-2B");
    }

    #[test]
    fn qwen25coder_ladder_fits_a_coder_on_16gb() {
        // Dense Qwen2 is lighter than Qwen3.5; a real coder should fit 16 GiB.
        let p = plan_qwen25coder(16.0, 7.0);
        assert!(!p.over_budget, "a Qwen2.5-Coder variant should fit 16 GiB");
        assert!(p.params_b >= 1.5, "should pick at least 1.5B on 16 GiB");
        assert!(p.model_id.contains("Qwen2.5-Coder"));
    }

    #[test]
    fn qwen25coder_scales_up() {
        let small = plan_qwen25coder(16.0, 32.0);
        let big = plan_qwen25coder(80.0, 32.0);
        assert!(big.params_b >= small.params_b);
    }

    #[test]
    fn qwen25coder_detection() {
        assert!(is_qwen25coder("Qwen/Qwen2.5-Coder-7B-Instruct"));
        assert!(!is_qwen25coder("Qwen/Qwen3.5-4B"));
        assert!(!is_qwen25coder("Qwen/Qwen2.5-7B-Instruct")); // non-coder qwen2.5
    }

    #[test]
    fn qwen35_family_detection() {
        assert!(is_qwen35("Qwen/Qwen3.5-4B"));
        assert!(is_qwen35("qwen3.5-2b"));
        assert!(!is_qwen35("meta-llama/Llama-3-8B"));
    }

    // NOTE: the VOX_MENS_VRAM_SAFETY env override is intentionally not unit-tested
    // here — mutating a process-global env var races with the other budget tests
    // under cargo's parallel test runner. The override is exercised in integration.
}
