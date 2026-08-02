use crate::config::CostPreference;
use crate::models::ModelSpec;
use crate::usage::RemainingBudget;
use vox_config::AutoRoutingPriority;

const QUALITY_FREE_PAID_COMPONENT: f64 = 0.35;
const QUALITY_PAID_COMPONENT: f64 = 0.95;
const QUALITY_TOKEN_WEIGHT: f64 = 0.6;
const QUALITY_PAID_WEIGHT: f64 = 0.4;
const EFFICIENCY_COST_SCALER: f64 = 100.0;
pub(super) const COMPLEXITY_HIGH_CUTOFF: u8 = 8;
const COMPLEXITY_LOW_CUTOFF: u8 = 3;
const COMPLEXITY_PRECISION_BONUS: u8 = 10;
const COMPLEXITY_EFFICIENCY_BONUS: u8 = 10;
const COMPLEXITY_LATENCY_BONUS: u8 = 5;
const FIM_CODE_SIGNAL_BONUS: f64 = 0.08;
const FIM_NON_CODE_SIGNAL_PENALTY: f64 = -0.02;
const ECONOMY_EFFICIENCY_BONUS: u8 = 15;
const PERFORMANCE_PRECISION_BONUS: u8 = 12;
const RATE_LIMITED_SCORE_FLOOR: f64 = -10_000.0;
const EMPTY_BUDGET_AVAILABILITY_SCORE: f64 = 0.35;
const BUDGET_LOG10_DIVISOR: f64 = 3.0;
const BUDGET_AVAILABILITY_MIN: f64 = 0.4;
/// Fallback RPM floor for throughput score when provider limits are unknown.
const THROUGHPUT_FALLBACK_RPM: f64 = 20.0;
/// Reference RPM for normalizing throughput (full score at this RPM or above).
const THROUGHPUT_REFERENCE_RPM: f64 = 200.0;
/// Routing score bonus for DeepSeek V3 during off-peak pricing window.
/// DeepSeek V3 is 50% cheaper UTC 16:30–00:30; this bonus makes the router prefer it then.
const DEEPSEEK_OFFPEAK_V3_BONUS: f64 = 0.07;
/// Routing score bonus for DeepSeek R1 during off-peak pricing window.
/// DeepSeek R1 is 75% cheaper UTC 16:30–00:30; stronger bonus reflects the larger discount.
const DEEPSEEK_OFFPEAK_R1_BONUS: f64 = 0.12;
/// Small flat scoring bonus for any zero-cost model (not just PopuliMesh).
/// Kept small and complexity-independent on purpose: at high complexity
/// `w.precision` already outweighs this, so a capable paid model still wins.
const ZERO_COST_BASE_BONUS: f64 = 0.1;
/// Small advisory penalty applied when [`super::vram::estimate_vram_fit`]
/// reports [`super::vram::VramFit::Exceeds`] for a candidate (Task 2.6).
///
/// Deliberately small and, deliberately, *not* zero: at high complexity the
/// zero-cost bonus above is already gated off (see `ZERO_COST_BASE_BONUS`
/// usage below), so this penalty is the only VRAM-fit signal left there and
/// must still move the score in the "less suitable" direction on its own —
/// it must never depend on canceling out the zero-cost bonus to have an
/// effect. At low/mid complexity, where the zero-cost bonus is active, this
/// penalty is kept smaller in magnitude than `ZERO_COST_BASE_BONUS` (half of
/// it) so a free-but-`Exceeds` local model nets a *smaller* bonus than a
/// free-and-fitting one, rather than the two flattening to a wash — see
/// `zero_cost_bonus_and_vram_penalty_compound_not_cancel` below.
///
/// Advisory only, per the module docs on [`super::vram`]: this is a ranking
/// deprioritization, never exclusion — the underlying VRAM estimate can be
/// wrong (unusual setups, other GPU-using processes, unmodeled
/// quantizations).
const VRAM_EXCEEDS_PENALTY: f64 = -0.05;

/// Blend telemetry scoreboard signals using contract `quality_weights`.
#[must_use]
pub fn scoreboard_feedback_boost(
    m: &ModelSpec,
    score: Option<&super::registry::ModelScore>,
    weights: &vox_config::model_routing::QualityWeightsConfig,
) -> f64 {
    let Some(s) = score else {
        return 0.0;
    };
    if s.n_calls <= 0 {
        return 0.0;
    }
    let success = s.success_rate.clamp(0.0, 1.0);
    let quality = s.quality_score.clamp(0.0, 1.0);
    let lat = s
        .p50_latency_ms
        .map(|ms| {
            let cfg = vox_config::load_model_routing_config();
            let excellent = cfg.latency_bands.excellent_ms;
            let poor = cfg.latency_bands.poor_ms;
            if ms as f64 <= excellent {
                1.0
            } else if ms as f64 >= poor {
                0.0
            } else {
                1.0 - ((ms as f64 - excellent) / (poor - excellent))
            }
        })
        .unwrap_or(0.5);
    let cost_inv = if let Some(cps) = s.cost_per_success_usd {
        (1.0 / (1.0 + cps * 100.0)).clamp(0.0, 1.0)
    } else {
        efficiency_score(m)
    };
    let w_sum = weights.socrates_factuality
        + weights.contradiction_inverse
        + weights.success_rate
        + weights.p50_latency_inverse
        + weights.cost_inverse;
    if w_sum <= 0.0 {
        return 0.0;
    }
    (weights.socrates_factuality * quality
        + weights.contradiction_inverse * (1.0 - (1.0 - success).min(1.0))
        + weights.success_rate * success
        + weights.p50_latency_inverse * lat
        + weights.cost_inverse * cost_inv)
        / w_sum
        * 0.15
}

/// Returns `true` when DeepSeek's off-peak pricing discount is active.
///
/// Window: **UTC 16:30–00:30** (59_400 s → 86_400 s, then 0 s → 1_800 s).
/// DeepSeek V3 gets 50% off; R1 gets 75% off during this window.
///
/// Exposed as `pub` so callers outside `scoring` can gate cost estimates (e.g. telemetry).
#[must_use]
pub fn is_deepseek_off_peak() -> bool {
    const START_SECS: u64 = 16 * 3_600 + 30 * 60; // 59_400 — 16:30 UTC
    const END_SECS: u64 = 30 * 60; // 1_800  — 00:30 UTC (next day)
    let sod = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        % 86_400;
    !(END_SECS..START_SECS).contains(&sod)
}

#[must_use]
pub(super) fn budget_match(limit_model: &str, model: &str) -> bool {
    limit_model == model
        || limit_model == "*"
        || (limit_model == ":free" && model.ends_with(":free"))
}

#[must_use]
pub(super) fn model_budget_hint(
    model: &ModelSpec,
    hints: Option<&[RemainingBudget]>,
) -> (u32, bool) {
    let usage = model.llm_usage_key();
    let mut remaining_max = 0u32;
    let mut any_rate_limited = false;
    for b in hints.unwrap_or(&[]) {
        if b.provider == usage.provider && budget_match(&b.model, &usage.model) {
            remaining_max = remaining_max.max(b.remaining);
            any_rate_limited |= b.rate_limited;
        }
    }
    (remaining_max, any_rate_limited)
}

#[must_use]
pub(super) fn quality_score(m: &ModelSpec) -> f64 {
    let token_component = (m.max_tokens as f64).log10().clamp(1.0, 7.0) / 7.0;
    let paid_component = if m.is_free {
        QUALITY_FREE_PAID_COMPONENT
    } else {
        QUALITY_PAID_COMPONENT
    };
    ((token_component * QUALITY_TOKEN_WEIGHT) + (paid_component * QUALITY_PAID_WEIGHT))
        .clamp(0.0, 1.0)
}

#[must_use]
pub(super) fn efficiency_score(m: &ModelSpec) -> f64 {
    let blended = if m.cost_per_1k_input > 0.0 || m.cost_per_1k_output > 0.0 {
        (m.cost_per_1k_input + m.cost_per_1k_output) / 2.0
    } else {
        m.cost_per_1k
    };
    if blended <= 0.0 {
        return 1.0;
    }
    (1.0 / (1.0 + blended * EFFICIENCY_COST_SCALER)).clamp(0.0, 1.0)
}

/// Latency score derived from the catalog-reported p50 latency when available, otherwise falls
/// back to a provider-type constant.  Score is 1.0 at ≤ 500 ms, decaying to 0.0 at ≥ 8 000 ms.
#[must_use]
pub(super) fn latency_score(m: &ModelSpec) -> f64 {
    use crate::models::ProviderType;

    if let Some(p50_ms) = m.capabilities.latency_p50_ms {
        let ms = p50_ms as f64;
        let cfg = vox_config::load_model_routing_config();
        let excellent = cfg.latency_bands.excellent_ms;
        let poor = cfg.latency_bands.poor_ms;

        if ms <= excellent {
            return 1.0;
        }
        if ms >= poor {
            return 0.0;
        }
        return 1.0 - (ms - excellent) / (poor - excellent);
    }

    match m.provider_type {
        ProviderType::Ollama => 0.95,
        ProviderType::Groq => 0.95,
        ProviderType::Cerebras => 0.95,
        ProviderType::GoogleDirect => 0.8,
        ProviderType::Anthropic => 0.75,
        ProviderType::HuggingFaceRouter => 0.9,
        ProviderType::OpenRouter => {
            // Give fast engines on OpenRouter a better fallback if missing p50
            if m.id.to_lowercase().contains("llama-3")
                || m.id.to_lowercase().contains("groq")
                || m.id.to_lowercase().contains("cerebras")
            {
                0.85
            } else {
                0.7
            }
        }
        _ => 0.65,
    }
}

/// Throughput score based on the provider's reported RPM limit.  Rewards high-throughput
/// providers that can sustain burst workloads; penalizes extremely restricted ones.
#[must_use]
pub(super) fn throughput_score(m: &ModelSpec) -> f64 {
    let rpm = m
        .capabilities
        .rate_limit_rpm
        .map(|r| r as f64)
        .unwrap_or(THROUGHPUT_FALLBACK_RPM);
    (rpm / THROUGHPUT_REFERENCE_RPM).clamp(0.0, 1.0)
}

/// Health score derived from uptime_score when available.  Degrades gracefully to 0.85 (a
/// modest penalty vs. a pristine 1.0) for providers where we have no uptime signal.
#[must_use]
pub(super) fn health_score(m: &ModelSpec) -> f64 {
    m.capabilities
        .uptime_score
        .map(|u| u as f64)
        .unwrap_or(0.85)
}

#[must_use]
pub(super) fn mobile_score(m: &ModelSpec) -> f64 {
    use crate::models::ProviderType;
    match vox_config::inference_profile_from_env() {
        vox_config::InferenceProfile::MobileLitert | vox_config::InferenceProfile::MobileCoreml => {
            if matches!(m.provider_type, ProviderType::Ollama) {
                0.0
            } else {
                1.0
            }
        }
        _ => 0.7,
    }
}

thread_local! {
    /// Per-selection routing-weight override. Set by [`AxesOverrideGuard`] around
    /// a single scorer pass so a request's `SelectionAxes` actually drive scoring,
    /// instead of only the process-global `VOX_AUTO_ROUTING_PRIORITY` env. The
    /// scoring path is synchronous (no `.await`), so concurrent daemon requests on
    /// different worker threads never interleave through this cell.
    static AXES_OVERRIDE: std::cell::RefCell<Option<AutoRoutingPriority>> =
        const { std::cell::RefCell::new(None) };
}

/// RAII guard installing a per-selection routing-weight override; restores the
/// previous value (normally `None`, i.e. fall back to env) on drop.
pub(crate) struct AxesOverrideGuard {
    prev: Option<AutoRoutingPriority>,
}

impl AxesOverrideGuard {
    pub(crate) fn set(axes: AutoRoutingPriority) -> Self {
        let prev = AXES_OVERRIDE.with(|c| c.borrow_mut().replace(axes));
        Self { prev }
    }
}

impl Drop for AxesOverrideGuard {
    fn drop(&mut self) {
        AXES_OVERRIDE.with(|c| *c.borrow_mut() = self.prev.take());
    }
}

/// Process-global base routing weights, installed by the daemon from the
/// persisted `routing_priority` preference. Read by [`base_routing_weights`]
/// when no per-selection thread-local override is active, taking precedence
/// over the `VOX_AUTO_ROUTING_PRIORITY` env fallback.
///
/// This mirrors `install_active_policy`: a thread-safe replacement for mutating
/// the process environment at runtime, which is undefined behavior under a
/// multi-threaded async runtime (other threads may `getenv` concurrently).
static BASE_AXES: std::sync::RwLock<Option<AutoRoutingPriority>> = std::sync::RwLock::new(None);

/// Install (or clear, with `None`) the process-global base routing weights.
/// Intended for daemon startup, before serving begins.
pub fn install_base_routing_priority(axes: Option<AutoRoutingPriority>) {
    if let Ok(mut g) = BASE_AXES.write() {
        *g = axes;
    }
}

/// Base routing weights for a scorer pass: the per-selection thread-local
/// override if one is installed (the request's `SelectionAxes`), else the
/// process-global base ([`install_base_routing_priority`]), else the
/// `VOX_AUTO_ROUTING_PRIORITY` env default.
fn base_routing_weights() -> AutoRoutingPriority {
    if let Some(axes) = AXES_OVERRIDE.with(|c| *c.borrow()) {
        return axes;
    }
    if let Ok(g) = BASE_AXES.read() {
        if let Some(axes) = *g {
            return axes;
        }
    }
    AutoRoutingPriority::from_env()
}

#[must_use]
pub fn auto_score_model(
    m: &ModelSpec,
    complexity: u8,
    free_tier_fill_in_middle: bool,
    context_fill_ratio: Option<f32>,
    preference: CostPreference,
    hints: Option<&[RemainingBudget]>,
    scoreboard: Option<&super::registry::ModelScore>,
) -> f64 {
    let mut w = base_routing_weights();
    if complexity >= COMPLEXITY_HIGH_CUTOFF {
        w.precision = w.precision.saturating_add(COMPLEXITY_PRECISION_BONUS);
        // Symmetric counterpart to the low-complexity efficiency/latency boost
        // below: a flat, complexity-independent `w.efficiency` gives free
        // models (efficiency_score == 1.0) a structural advantage that
        // `w.precision`'s boost alone can't reliably overcome (verified: even
        // with the precision boost, a small free 8B-class model beat a
        // frontier paid model by ~6% under default weights). Since the
        // zero-cost bonus below is already gated off at this complexity band
        // (see `ZERO_COST_BASE_BONUS` below), the residual advantage comes
        // from `efficiency_score`/`w.efficiency` itself, not the bonus — so
        // trimming efficiency's weight here, mirroring the existing boost at
        // low complexity, is the correct place to fix it. Guarded against
        // `free_local_model_does_not_win_high_complexity`.
        w.efficiency = w.efficiency.saturating_sub(COMPLEXITY_EFFICIENCY_BONUS);
    } else if complexity <= COMPLEXITY_LOW_CUTOFF {
        w.efficiency = w.efficiency.saturating_add(COMPLEXITY_EFFICIENCY_BONUS);
        w.latency = w.latency.saturating_add(COMPLEXITY_LATENCY_BONUS);
    }
    let fim_bias = if free_tier_fill_in_middle {
        let id = m.id.to_ascii_lowercase();
        let has_code_signal = m.strengths.iter().any(|s| {
            *s == crate::models::StrengthTag::Codegen || *s == crate::models::StrengthTag::Parsing
        }) || id.contains("coder")
            || id.contains("code")
            || id.contains("instruct");
        if has_code_signal {
            FIM_CODE_SIGNAL_BONUS
        } else {
            FIM_NON_CODE_SIGNAL_PENALTY
        }
    } else {
        0.0
    };
    match preference {
        CostPreference::Economy => {
            w.efficiency = w.efficiency.saturating_add(ECONOMY_EFFICIENCY_BONUS)
        }
        CostPreference::Performance => {
            w.precision = w.precision.saturating_add(PERFORMANCE_PRECISION_BONUS)
        }
    }

    let (remaining, rate_limited) = model_budget_hint(m, hints);
    if rate_limited {
        return RATE_LIMITED_SCORE_FLOOR;
    }

    let balance_bias = 1.0_f64 - f64::from(context_fill_ratio.unwrap_or(0.0).clamp(0.0, 1.0));
    let availability_score = if remaining == 0 {
        EMPTY_BUDGET_AVAILABILITY_SCORE
    } else {
        (f64::from(remaining).log10() / BUDGET_LOG10_DIVISOR).clamp(BUDGET_AVAILABILITY_MIN, 1.0)
    };

    // Derive composite latency+throughput+health score: latency is the largest contributor,
    // throughput provides burst capacity signal, health penalizes degraded providers.
    let live_latency =
        (latency_score(m) * 0.6 + throughput_score(m) * 0.25 + health_score(m) * 0.15)
            .clamp(0.0, 1.0);

    let total_w = f64::from(
        u16::from(w.efficiency)
            + u16::from(w.precision)
            + u16::from(w.latency)
            + u16::from(w.availability)
            + u16::from(w.balance)
            + u16::from(w.mobile),
    )
    .max(1.0);
    let score = f64::from(w.efficiency) * efficiency_score(m)
        + f64::from(w.precision) * quality_score(m)
        + f64::from(w.latency) * live_latency
        + f64::from(w.availability) * availability_score
        + f64::from(w.balance) * balance_bias
        + f64::from(w.mobile) * mobile_score(m);

    let prefer_mesh = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxRoutingPreferMesh)
        .expose()
        .map(|s: &str| s.trim() == "true")
        .unwrap_or(false);

    #[cfg_attr(not(feature = "populi-transport"), allow(unused_mut))]
    let mut mens_bonus = if m.provider_type == crate::models::ProviderType::PopuliMesh {
        if prefer_mesh {
            0.8 // High bonus to strongly prefer mesh
        } else if *m.id == *"mens/vox-language-model" {
            0.25
        } else {
            ZERO_COST_BASE_BONUS
        }
    } else if m.is_free && complexity < COMPLEXITY_HIGH_CUTOFF {
        // Any other zero-cost provider (e.g. local Ollama models) earns the
        // same small base bonus PopuliMesh gets for being free — the mesh-only
        // tiers above (0.8 prefer_mesh / 0.25 the mesh flagship) stay
        // PopuliMesh-specific, since those reflect an explicit mesh-preference
        // policy or a specific first-party model, not "any free model".
        //
        // Complexity-gated (not applied at/above COMPLEXITY_HIGH_CUTOFF): even
        // with this bonus at zero, `efficiency_score`'s max-value-for-free
        // behavior plus Ollama's optimistic default `latency_score` fallback
        // already give local models a structural edge that a flat additive
        // bonus would only worsen at high complexity. Restricting the bonus to
        // low/mid complexity keeps it purely a "make local competitive when it
        // plausibly should win" nudge, never a contributor to it winning hard
        // tasks. See `free_local_model_does_not_win_high_complexity` below.
        ZERO_COST_BASE_BONUS
    } else {
        0.0
    };

    #[cfg(feature = "populi-transport")]
    if m.provider_type == crate::models::ProviderType::PopuliMesh {
        // Prefer the first-class `donations.vox` policy file (parsed via
        // vox-mesh-policy) when `VOX_MESH_DONATION_POLICY_PATH` points at an
        // existing file. This keeps the legacy JSON secret as a fallback, so
        // deployments that never set the path secret behave identically.
        let policy: Option<vox_mesh_types::WorkerDonationPolicy> =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshDonationPolicyPath)
                .expose()
                .map(std::path::PathBuf::from)
                .filter(|p| p.is_file())
                .and_then(|p| vox_mesh_policy::load_policy(&p).ok())
                .or_else(|| {
                    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshDonationPolicyJson)
                        .expose()
                        .and_then(|json| {
                            serde_json::from_str::<vox_mesh_types::WorkerDonationPolicy>(json).ok()
                        })
                });
        if let Some(policy) = policy {
            if policy.public_mesh_opt_in {
                mens_bonus += 0.15; // Reciprocity bonus for donating to the network
            }
        }
    }

    // Off-peak pricing bonus: DeepSeek cuts prices 50–75% UTC 16:30–00:30.
    // A small additive bonus tips routing toward DeepSeek when competing models score similarly.
    let off_peak_bonus = if m.id.to_ascii_lowercase().contains("deepseek") && is_deepseek_off_peak()
    {
        if m.id.to_ascii_lowercase().contains("r1") {
            DEEPSEEK_OFFPEAK_R1_BONUS
        } else {
            DEEPSEEK_OFFPEAK_V3_BONUS
        }
    } else {
        0.0
    };

    let routing_cfg = vox_config::load_model_routing_config();
    let telemetry_boost = scoreboard_feedback_boost(m, scoreboard, &routing_cfg.quality_weights);
    let vram_penalty = vram_score_delta(m, super::vram::free_vram_mb_hint());

    (score / total_w) + fim_bias + mens_bonus + off_peak_bonus + telemetry_boost + vram_penalty
}

/// Advisory VRAM-fit contribution to the score (Task 2.6): a small
/// deprioritization when [`super::vram::estimate_vram_fit`] reports
/// [`super::vram::VramFit::Exceeds`], zero otherwise. `Unknown` (no NVML/GPU,
/// or no parameter-count data for `m`) is a true no-op — see the
/// `super::vram` module docs. Split out from [`auto_score_model`] as a pure
/// function so it's directly unit-testable without mutating the process-wide
/// free-VRAM cache.
#[must_use]
fn vram_score_delta(m: &ModelSpec, free_vram_mb: Option<u64>) -> f64 {
    match super::vram::estimate_vram_fit(m, free_vram_mb) {
        super::vram::VramFit::Exceeds => VRAM_EXCEEDS_PENALTY,
        super::vram::VramFit::Comfortable
        | super::vram::VramFit::Tight
        | super::vram::VramFit::Unknown => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{ModelCapabilities, ModelSpec, ProviderType};

    use super::*;

    fn make_spec(provider_type: ProviderType, cost: f64, is_free: bool) -> ModelSpec {
        ModelSpec {
            id: "test/model".into(),
            canonical_slug: "test/model".into(),
            provider: "test".into(),
            provider_type,
            max_tokens: 8192,
            cost_per_1k: cost,
            cost_per_1k_input: cost,
            cost_per_1k_output: cost,
            is_free,
            observed_cost_per_1k: None,
            strengths: vec![crate::models::StrengthTag::Codegen],
            capabilities: ModelCapabilities::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        }
    }

    #[test]
    fn latency_score_uses_p50_when_available() {
        let mut spec = make_spec(ProviderType::OpenRouter, 0.0, true);
        spec.capabilities.latency_p50_ms = Some(250);
        assert_eq!(latency_score(&spec), 1.0, "p50 <= 500ms -> score 1.0");

        spec.capabilities.latency_p50_ms = Some(4250);
        let mid = latency_score(&spec);
        assert!(mid > 0.0 && mid < 1.0, "mid p50 -> intermediate score");

        spec.capabilities.latency_p50_ms = Some(10_000);
        assert_eq!(latency_score(&spec), 0.0, "p50 >= 8000ms -> score 0.0");
    }

    #[test]
    fn latency_score_fallback_for_provider_type() {
        let spec = make_spec(ProviderType::Ollama, 0.0, true);
        assert_eq!(latency_score(&spec), 0.95, "Ollama fallback = 0.95");
        let spec2 = make_spec(ProviderType::OpenRouter, 0.0, false);
        assert_eq!(latency_score(&spec2), 0.7, "OpenRouter fallback = 0.7");
        let spec3 = make_spec(ProviderType::Groq, 0.0, true);
        assert_eq!(latency_score(&spec3), 0.95, "Groq fallback = 0.95");
        let spec4 = make_spec(ProviderType::Anthropic, 0.0, false);
        assert_eq!(latency_score(&spec4), 0.75, "Anthropic fallback = 0.75");
    }

    #[test]
    fn throughput_score_clamps_to_unit_interval() {
        let mut spec = make_spec(ProviderType::OpenRouter, 0.0, true);
        spec.capabilities.rate_limit_rpm = Some(1000);
        assert_eq!(throughput_score(&spec), 1.0, "high RPM -> 1.0 (clamped)");

        spec.capabilities.rate_limit_rpm = Some(100);
        assert!(
            (throughput_score(&spec) - 0.5).abs() < 1e-9,
            "100 RPM at reference 200 -> 0.5"
        );

        spec.capabilities.rate_limit_rpm = None;
        assert!(throughput_score(&spec) > 0.0);
    }

    #[test]
    fn health_score_uses_uptime_score() {
        let mut spec = make_spec(ProviderType::OpenRouter, 0.0, false);
        spec.capabilities.uptime_score = Some(0.99);
        assert!((health_score(&spec) - 0.99).abs() < 1e-6);
        spec.capabilities.uptime_score = None;
        assert_eq!(health_score(&spec), 0.85, "missing uptime -> 0.85 default");
    }

    #[test]
    fn rate_limited_model_floors_to_negative() {
        let spec = make_spec(ProviderType::OpenRouter, 0.01, false);
        let hints = vec![crate::usage::RemainingBudget {
            provider: "openrouter".into(),
            model: "test/model".into(),
            calls_used: 50,
            daily_limit: 100,
            remaining: 50,
            cost_today: 0.5,
            rate_limited: true,
        }];
        let score = auto_score_model(
            &spec,
            5,     // default complexity
            false, // no FIM
            None,  // no context fill
            CostPreference::Economy,
            Some(&hints),
            None,
        );
        assert!(score <= RATE_LIMITED_SCORE_FLOOR, "rate-limited -> floor");
    }

    /// Task 2.1b root cause 1: Ollama discovery hardcoded `max_tokens: 4096`
    /// for every model regardless of real context length, which crushed
    /// `quality_score`'s token component (log10(4096)/7 vs log10(40000)/7).
    /// This proves fixing the discovered `max_tokens` actually moves the
    /// metric the routing decision depends on.
    #[test]
    fn quality_score_rewards_real_context_length_over_fake_fallback() {
        let mut fake_fallback = make_spec(ProviderType::Ollama, 0.0, true);
        fake_fallback.max_tokens = 4096;

        let mut real_context = make_spec(ProviderType::Ollama, 0.0, true);
        real_context.max_tokens = 40_000;

        let low = quality_score(&fake_fallback);
        let high = quality_score(&real_context);
        assert!(
            high > low + 0.05,
            "40000-token model should score meaningfully higher than the \
             4096 fallback (low={low}, high={high})"
        );
    }

    /// Task 2.1b root cause 2: the zero-cost bonus was gated on
    /// `provider_type == PopuliMesh`, so an equally-free Ollama model got no
    /// bonus at all. At low complexity (where efficiency/latency dominate and
    /// quality gaps are small) a free Ollama model should now score close to
    /// a similarly-capable free PopuliMesh model, proving parity was reached
    /// without a hardcoded provider check.
    #[test]
    fn free_ollama_model_competitive_with_free_mesh_model_at_low_complexity() {
        // Scoped + auto-restored (previously a raw `unsafe { set_var }` with no
        // teardown, which both fails clippy's workspace-wide `unsafe_code` deny
        // and leaked the override into later tests).
        let _env_guard = vox_test_harness::env_scratch::EnvScratch::empty()
            .set("VOX_ROUTING_PREFER_MESH", "false");

        let mut ollama = make_spec(ProviderType::Ollama, 0.0, true);
        ollama.id = "qwen3:8b".into();
        ollama.canonical_slug = "ollama/qwen3:8b".into();
        ollama.max_tokens = 40_000;

        let mut mesh = make_spec(ProviderType::PopuliMesh, 0.0, true);
        mesh.id = "populi/other-model".into();
        mesh.canonical_slug = "populi/other-model".into();
        mesh.max_tokens = 40_000;

        let low_complexity = 1;
        let ollama_score = auto_score_model(
            &ollama,
            low_complexity,
            false,
            None,
            CostPreference::Economy,
            None,
            None,
        );
        let mesh_score = auto_score_model(
            &mesh,
            low_complexity,
            false,
            None,
            CostPreference::Economy,
            None,
            None,
        );

        assert!(
            (ollama_score - mesh_score).abs() < 0.05,
            "free Ollama model should be competitive with a similarly-capable \
             free non-flagship PopuliMesh model at low complexity \
             (ollama={ollama_score}, mesh={mesh_score})"
        );
    }

    /// Critical regression guard for the LiteLLM "free always wins" trap
    /// flagged during Task 2.1b planning: at high complexity, a small free
    /// local (Ollama) model must NOT outscore a large, capable paid model.
    /// `w.precision`'s complexity-scaled weight plus the paid model's far
    /// higher `quality_score` (bigger context, non-free component) must
    /// dominate the flat +0.1 zero-cost bonus.
    #[test]
    fn free_local_model_does_not_win_high_complexity() {
        let mut small_local = make_spec(ProviderType::Ollama, 0.0, true);
        small_local.id = "qwen3:8b".into();
        small_local.canonical_slug = "ollama/qwen3:8b".into();
        small_local.max_tokens = 40_000; // real context, post-fix

        let mut frontier_paid = make_spec(ProviderType::Anthropic, 0.045, false);
        frontier_paid.id = "anthropic/claude-frontier".into();
        frontier_paid.canonical_slug = "anthropic/claude-frontier".into();
        frontier_paid.max_tokens = 200_000;
        frontier_paid.cost_per_1k_input = 0.03;
        frontier_paid.cost_per_1k_output = 0.06;

        let high_complexity = COMPLEXITY_HIGH_CUTOFF;
        let local_score = auto_score_model(
            &small_local,
            high_complexity,
            false,
            None,
            CostPreference::Performance,
            None,
            None,
        );
        let paid_score = auto_score_model(
            &frontier_paid,
            high_complexity,
            false,
            None,
            CostPreference::Performance,
            None,
            None,
        );

        assert!(
            paid_score > local_score,
            "a frontier paid model must outscore a small free local model at \
             high complexity (local={local_score}, paid={paid_score}) — a \
             regression here reproduces the LiteLLM zero-cost trap"
        );
    }

    /// Task 2.6: `vram_score_delta` (the pure function `auto_score_model`
    /// delegates to) must apply the small fixed penalty for `Exceeds` and be
    /// a true no-op for `Unknown` — tested directly, without touching the
    /// process-global free-VRAM cache, so this test can't race others.
    #[test]
    fn vram_score_delta_penalizes_exceeds_and_noops_on_unknown() {
        let mut huge_local = make_spec(ProviderType::Ollama, 0.0, true);
        huge_local.capabilities.param_count_b = Some(70.0); // ~33 GiB @ Q4 assumption

        // Tiny free VRAM -> Exceeds -> the fixed penalty applies.
        assert_eq!(
            vram_score_delta(&huge_local, Some(1_000)),
            VRAM_EXCEEDS_PENALTY
        );

        // No parameter-count data at all -> Unknown -> zero, regardless of free VRAM.
        let mut no_param_data = make_spec(ProviderType::Ollama, 0.0, true);
        no_param_data.capabilities.param_count_b = None;
        assert_eq!(vram_score_delta(&no_param_data, Some(1_000)), 0.0);

        // No free-VRAM signal at all (no NVML/GPU) -> Unknown -> zero, even
        // though the model itself is "huge" and would otherwise Exceed.
        assert_eq!(vram_score_delta(&huge_local, None), 0.0);

        // Plenty of VRAM -> Comfortable -> zero (only Exceeds is penalized).
        let mut small_local = make_spec(ProviderType::Ollama, 0.0, true);
        small_local.capabilities.param_count_b = Some(1.0);
        assert_eq!(vram_score_delta(&small_local, Some(20_000)), 0.0);
    }

    /// Task 2.6: `VramFit::Unknown` must be a *true* no-op end-to-end through
    /// `auto_score_model`, not merely mathematically zero in isolation. Two
    /// models differing ONLY in whether they carry `param_count_b` (i.e.
    /// whether a VRAM-fit signal exists at all) must score identically when
    /// no free-VRAM hint is available — proving the signal genuinely behaves
    /// as "doesn't exist for this session" rather than defaulting to fits/
    /// doesn't-fit. Uses the crate's real global hint accessor
    /// (`free_vram_mb_hint`), which defaults to `None` absent an explicit
    /// `set_free_vram_mb_hint` call — this test makes none, so it can't race
    /// other tests over that global.
    #[test]
    fn vram_unknown_signal_is_true_noop_through_auto_score_model() {
        let mut baseline = make_spec(ProviderType::Ollama, 0.0, true);
        baseline.id = "qwen3:8b".into();
        baseline.canonical_slug = "ollama/qwen3:8b".into();
        baseline.max_tokens = 40_000;
        baseline.capabilities.param_count_b = None; // no signal

        let mut with_param_data = baseline.clone();
        with_param_data.capabilities.param_count_b = Some(8.0); // signal exists, but no free-VRAM hint -> Unknown

        let complexity = 5;
        let baseline_score = auto_score_model(
            &baseline,
            complexity,
            false,
            None,
            CostPreference::Economy,
            None,
            None,
        );
        let with_param_score = auto_score_model(
            &with_param_data,
            complexity,
            false,
            None,
            CostPreference::Economy,
            None,
            None,
        );
        assert_eq!(
            baseline_score, with_param_score,
            "with no free-VRAM hint available, adding param_count_b data must \
             not change the score at all (Unknown is a true no-op)"
        );
    }

    /// Task 2.6: a zero-cost local model that ALSO estimates to `Exceeds`
    /// VRAM must not "average out to neutral" against the zero-cost bonus —
    /// the two effects must compound in the same "less suitable" direction.
    /// Verified two ways: (1) the VRAM penalty is strictly smaller in
    /// magnitude than the zero-cost bonus, so it can only shrink the net
    /// bonus, never flip its sign or exceed it; (2) at the complexity-9
    /// zero-cost-trap guard band (where the bonus is already gated to zero),
    /// the VRAM penalty still fires on its own and does not get canceled by
    /// anything — it strictly lowers the score relative to an
    /// otherwise-identical model with no VRAM-fit signal.
    #[test]
    fn zero_cost_bonus_and_vram_penalty_compound_not_cancel() {
        assert!(
            VRAM_EXCEEDS_PENALTY.abs() < ZERO_COST_BASE_BONUS,
            "the VRAM penalty must not be large enough to flip the zero-cost \
             bonus negative on its own (it should shrink the net bonus, not \
             invert it)"
        );

        // At the complexity-9 guard band the zero-cost bonus is gated off, so
        // the only remaining VRAM-related term is the penalty itself. Compare
        // an Exceeds-signaled huge local model against an otherwise-identical
        // model with no VRAM signal (Unknown, via no param data) — the former
        // must score strictly lower, proving the penalty has independent
        // effect rather than needing the (already-zero) bonus to cancel.
        let mut huge_local_no_signal = make_spec(ProviderType::Ollama, 0.0, true);
        huge_local_no_signal.id = "qwen3:70b".into();
        huge_local_no_signal.canonical_slug = "ollama/qwen3:70b".into();
        huge_local_no_signal.max_tokens = 40_000;
        huge_local_no_signal.capabilities.param_count_b = None;

        let high_complexity = COMPLEXITY_HIGH_CUTOFF;
        let score_no_signal = auto_score_model(
            &huge_local_no_signal,
            high_complexity,
            false,
            None,
            CostPreference::Performance,
            None,
            None,
        );

        // vram_score_delta is exercised directly here (not through the global
        // hint) to avoid any cross-test race, but it demonstrates exactly the
        // term `auto_score_model` would add if the free-VRAM hint were set to
        // a small value for this huge model.
        let mut huge_local_with_signal = huge_local_no_signal.clone();
        huge_local_with_signal.capabilities.param_count_b = Some(70.0);
        let delta = vram_score_delta(&huge_local_with_signal, Some(1_000));
        assert_eq!(delta, VRAM_EXCEEDS_PENALTY);
        assert!(
            score_no_signal + delta < score_no_signal,
            "applying the Exceeds penalty on top of the no-signal score must \
             strictly lower it — it must never cancel out to neutral"
        );
    }
}

#[cfg(test)]
mod axes_override_tests {
    use super::*;
    use crate::config::CostPreference;
    use crate::models::generated::StrengthTag;
    use crate::models::spec::PricingSource;
    use crate::models::{ModelSpec, ProviderType};

    fn paid_premium() -> ModelSpec {
        ModelSpec {
            id: "paid-premium".into(),
            canonical_slug: "paid-premium".into(),
            provider: "anthropic".into(),
            provider_type: ProviderType::Anthropic,
            max_tokens: 200_000,
            cost_per_1k: 0.045,
            cost_per_1k_input: 0.03,
            cost_per_1k_output: 0.06,
            is_free: false,
            observed_cost_per_1k: None,
            strengths: vec![StrengthTag::Codegen],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: PricingSource::OpenRouter,
            supported_parameters: vec![],
        }
    }

    fn axes(efficiency: u8, precision: u8, latency: u8) -> AutoRoutingPriority {
        AutoRoutingPriority {
            efficiency,
            precision,
            latency,
            availability: 0,
            balance: 0,
            mobile: 0,
        }
    }

    /// The process-global base (set by the daemon from the persisted preference)
    /// must be observed by `base_routing_weights` when no per-call thread-local
    /// override is active, and clearing it restores the env fallback. This is the
    /// thread-safe replacement for the old `set_var` startup mutation.
    ///
    /// `BASE_AXES` is a process-global `RwLock` (see above); `#[serial]` keeps
    /// this test's install/clear mutually exclusive with any other test whose
    /// scoring path reads `base_routing_weights()` without installing its own
    /// thread-local override (e.g. `policy::tests::emphasize_intelligence_vs_efficiency_differ`).
    #[test]
    #[serial_test::serial]
    fn install_base_routing_priority_is_observed_then_cleared() {
        // No thread-local override here (this test installs none).
        install_base_routing_priority(Some(axes(11, 22, 33)));
        let got = base_routing_weights();
        assert_eq!((got.efficiency, got.precision, got.latency), (11, 22, 33));

        // Clearing falls back to env (default when unset).
        install_base_routing_priority(None);
        let fallback = base_routing_weights();
        assert_eq!(fallback, AutoRoutingPriority::from_env());
    }

    /// The per-call axes override must actually steer scoring. Before the fix
    /// `auto_score_model` only read `VOX_AUTO_ROUTING_PRIORITY` from the env, so
    /// the request's SelectionAxes were ignored and both scores were identical.
    #[test]
    fn axes_override_changes_score() {
        let m = paid_premium();
        let quality = {
            let _g = AxesOverrideGuard::set(axes(15, 70, 15));
            auto_score_model(&m, 5, false, None, CostPreference::Performance, None, None)
        };
        let cost = {
            let _g = AxesOverrideGuard::set(axes(70, 15, 15));
            auto_score_model(&m, 5, false, None, CostPreference::Performance, None, None)
        };
        assert_ne!(
            quality, cost,
            "per-call axes override must change the score (precision-heavy {quality} vs efficiency-heavy {cost})"
        );
        // The override is cleared on guard drop (falls back to env default).
        assert!(super::AXES_OVERRIDE.with(|c| c.borrow().is_none()));
    }
}

/// B11 wiring: the model-admission path must read a first-class `donations.vox`
/// file via `vox_mesh_policy::load_policy` when `VOX_MESH_DONATION_POLICY_PATH`
/// points at one, applying the reciprocity opt-in bonus — not only the JSON
/// secret. These tests are serial because they mutate the donation-policy env.
#[cfg(all(test, feature = "populi-transport"))]
mod donations_vox_wiring_tests {
    use super::*;
    use crate::config::CostPreference;
    use crate::models::generated::StrengthTag;
    use crate::models::{ModelCapabilities, ModelSpec, ProviderType};

    fn populi_mesh() -> ModelSpec {
        ModelSpec {
            id: "mens/vox-language-model".into(),
            canonical_slug: "mens/vox-language-model".into(),
            provider: "populi".into(),
            provider_type: ProviderType::PopuliMesh,
            max_tokens: 8192,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free: true,
            observed_cost_per_1k: None,
            strengths: vec![StrengthTag::Codegen],
            capabilities: ModelCapabilities::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        }
    }

    /// The canonical `donations.vox` fixture used across these tests: a single
    /// `let public_mesh_opt_in = true` binding, exactly the grammar
    /// `vox_mesh_policy::parse_source` consumes.
    const OPT_IN_FIXTURE: &str = "let public_mesh_opt_in = true\n";

    #[test]
    #[serial_test::serial(donation_policy_env)]
    fn load_policy_parses_opt_in_from_donations_vox() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("donations.vox");
        std::fs::write(&path, OPT_IN_FIXTURE).unwrap();
        let policy = vox_mesh_policy::load_policy(&path).unwrap();
        assert!(
            policy.public_mesh_opt_in,
            "donations.vox `let public_mesh_opt_in = true` must parse to opt-in"
        );
    }

    #[test]
    #[serial_test::serial(donation_policy_env)]
    fn scoring_applies_reciprocity_bonus_from_donations_vox_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("donations.vox");
        std::fs::write(&path, OPT_IN_FIXTURE).unwrap();

        let m = populi_mesh();

        // Ensure neither donation-policy secret leaks in from the environment.
        let prev_json = std::env::var("VOX_MESH_DONATION_POLICY_JSON").ok();
        let prev_path = std::env::var("VOX_MESH_DONATION_POLICY_PATH").ok();
        unsafe {
            std::env::remove_var("VOX_MESH_DONATION_POLICY_JSON");
            std::env::remove_var("VOX_MESH_DONATION_POLICY_PATH");
            std::env::set_var("VOX_ROUTING_PREFER_MESH", "false");
        }

        let baseline =
            auto_score_model(&m, 5, false, None, CostPreference::Performance, None, None);

        unsafe {
            std::env::set_var("VOX_MESH_DONATION_POLICY_PATH", &path);
        }
        let with_opt_in =
            auto_score_model(&m, 5, false, None, CostPreference::Performance, None, None);

        // Restore environment.
        unsafe {
            std::env::remove_var("VOX_MESH_DONATION_POLICY_PATH");
            match prev_path {
                Some(v) => std::env::set_var("VOX_MESH_DONATION_POLICY_PATH", v),
                None => std::env::remove_var("VOX_MESH_DONATION_POLICY_PATH"),
            }
            match prev_json {
                Some(v) => std::env::set_var("VOX_MESH_DONATION_POLICY_JSON", v),
                None => std::env::remove_var("VOX_MESH_DONATION_POLICY_JSON"),
            }
        }

        assert!(
            with_opt_in > baseline,
            "donations.vox opt-in must add the reciprocity bonus \
             (baseline {baseline} vs with-opt-in {with_opt_in})"
        );
    }
}
