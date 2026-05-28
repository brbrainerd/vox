use crate::config::CostPreference;
use crate::models::ModelSpec;
use crate::usage::RemainingBudget;
use vox_config::AutoRoutingPriority;

const QUALITY_FREE_PAID_COMPONENT: f64 = 0.35;
const QUALITY_PAID_COMPONENT: f64 = 0.95;
const QUALITY_TOKEN_WEIGHT: f64 = 0.6;
const QUALITY_PAID_WEIGHT: f64 = 0.4;
const EFFICIENCY_COST_SCALER: f64 = 100.0;
const COMPLEXITY_HIGH_CUTOFF: u8 = 8;
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

#[must_use]
pub fn auto_score_model(
    m: &ModelSpec,
    complexity: u8,
    free_tier_fill_in_middle: bool,
    context_fill_ratio: Option<f32>,
    preference: CostPreference,
    hints: Option<&[RemainingBudget]>,
) -> f64 {
    let mut w = AutoRoutingPriority::from_env();
    if complexity >= COMPLEXITY_HIGH_CUTOFF {
        w.precision = w.precision.saturating_add(COMPLEXITY_PRECISION_BONUS);
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
            0.1 // Base bonus for zero cost
        }
    } else {
        0.0
    };

    #[cfg(feature = "populi-transport")]
    if m.provider_type == crate::models::ProviderType::PopuliMesh {
        if let Some(json) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshDonationPolicyJson).expose()
        {
            if let Ok(policy) = serde_json::from_str::<vox_mesh_types::WorkerDonationPolicy>(json) {
                if policy.public_mesh_opt_in {
                    mens_bonus += 0.15; // Reciprocity bonus for donating to the network
                }
            }
        }
    }

    // Off-peak pricing bonus: DeepSeek cuts prices 50–75% UTC 16:30–00:30.
    // A small additive bonus tips routing toward DeepSeek when competing models score similarly.
    let off_peak_bonus = if m.id.to_ascii_lowercase().contains("deepseek")
        && is_deepseek_off_peak()
    {
        if m.id.to_ascii_lowercase().contains("r1") {
            DEEPSEEK_OFFPEAK_R1_BONUS
        } else {
            DEEPSEEK_OFFPEAK_V3_BONUS
        }
    } else {
        0.0
    };

    (score / total_w) + fim_bias + mens_bonus + off_peak_bonus
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
        );
        assert!(score <= RATE_LIMITED_SCORE_FLOOR, "rate-limited -> floor");
    }
}
