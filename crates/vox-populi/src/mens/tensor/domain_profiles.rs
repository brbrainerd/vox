use crate::mens::tensor::training_config::{ContextFilter, CurriculumSchedule};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Training method selected per spoke. Mirrors the methods our trainer can
/// dispatch; extend ONLY when the trainer gains a real backend (no stubs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainMethod {
    Qlora,
    FullSft,
    Dpo,
    Orpo,
    /// No fine-tune: spoke is served via retrieval/prompting only.
    RagOnly,
    PromptOnly,
}

/// Per-spoke base model + training method + hardware preset.
/// `model` and `preset` are validated against `model-registry.yaml` /
/// `gpu-specs.yaml` by `spoke_validate` (Phase 1.4) — a typo fails arch-check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpokeBase {
    pub model: String,
    pub method: TrainMethod,
    #[serde(default)]
    pub preset: Option<String>,
}

/// Inference-time routing hints for this spoke. `triggers`/`priority` are
/// consumed by `route_by_signal` (lane → spoke). `prefer_local` is a
/// forward-looking flag for Phase-7 local-vs-cloud inference routing; it has no
/// consumer yet — the routing function will be added together with that
/// consumer (and an end-to-end test), not speculatively ahead of it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpokeRouter {
    /// Lane tags / glob triggers that route a request to this spoke.
    #[serde(default)]
    pub triggers: Vec<String>,
    /// Higher wins when multiple spokes match.
    #[serde(default)]
    pub priority: i32,
    /// Phase-7 hint: prefer the spoke's local fine-tuned adapter over a cloud
    /// model when one exists. No consumer yet (see struct-level note).
    #[serde(default)]
    pub prefer_local: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainProfile {
    pub description: Option<String>,
    pub context_filter: Option<ContextFilter>,
    pub mix_config: Option<String>,
    #[serde(default)]
    pub base: Option<SpokeBase>,
    #[serde(default)]
    pub eval_gate: Option<String>,
    #[serde(default)]
    pub router: Option<SpokeRouter>,
    pub system_prompt: Option<String>,
    pub min_rating: Option<u8>,
    pub ce_last_k: Option<usize>,
    pub seq_len: Option<usize>,
    pub max_grad_norm: Option<f32>,
    pub trajectory_weighting: Option<bool>,
    pub trajectory_tool_trace_boost: Option<f32>,
    pub curriculum_schedule: Option<CurriculumSchedule>,
    pub chatml: Option<crate::mens::tensor::training_config::ChatmlConfig>,
    pub reward_hook: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainProfileDefaults {
    pub min_rating: Option<u8>,
    pub ce_last_k: Option<usize>,
    pub seq_len: Option<usize>,
    pub validation_split_ratio: Option<f64>,
    pub weight_decay: Option<f32>,
    pub max_grad_norm: Option<f32>,
    pub curriculum: Option<bool>,
    pub trajectory_weighting: Option<bool>,
    pub curriculum_schedule: Option<CurriculumSchedule>,
    pub chatml: Option<crate::mens::tensor::training_config::ChatmlConfig>,
    pub reward_hook: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainProfilesFile {
    pub defaults: Option<DomainProfileDefaults>,
    pub profiles: HashMap<String, DomainProfile>,
}

impl DomainProfilesFile {
    /// Read and parse mens/config/domain-profiles.yaml from the workspace root.
    pub fn load(workspace_root: Option<&Path>) -> anyhow::Result<Self> {
        let root = workspace_root.unwrap_or_else(|| Path::new("."));
        let profiles_path = root.join("mens/config/domain-profiles.yaml");
        let content = std::fs::read_to_string(&profiles_path)
            .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", profiles_path.display(), e))?;
        serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse domain profiles: {}", e))
    }
}

#[derive(Debug, Clone)]
pub struct EffectiveDomainProfile {
    pub name: String,
    pub description: Option<String>,
    pub context_filter: Option<ContextFilter>,
    pub mix_config: Option<PathBuf>,
    pub system_prompt: Option<PathBuf>,
    pub base: Option<SpokeBase>,
    pub eval_gate: Option<PathBuf>,
    pub router: Option<SpokeRouter>,

    // Overrides over LoraTrainingConfig defaults
    pub min_rating: Option<u8>,
    pub ce_last_k: Option<usize>,
    pub seq_len: Option<usize>,
    pub validation_split_ratio: Option<f64>,
    pub weight_decay: Option<f32>,
    pub max_grad_norm: Option<f32>,
    pub curriculum: Option<bool>,
    pub trajectory_weighting: Option<bool>,
    pub trajectory_tool_trace_boost: Option<f32>,
    pub curriculum_schedule: Option<CurriculumSchedule>,
    pub chatml: crate::mens::tensor::training_config::ChatmlConfig,
    pub reward_hook: Option<String>,
}

impl EffectiveDomainProfile {
    pub fn load_domain_profile(name: &str, workspace_root: Option<&Path>) -> anyhow::Result<Self> {
        let root = workspace_root.unwrap_or_else(|| Path::new("."));
        let file = DomainProfilesFile::load(workspace_root)?;

        let profile = file.profiles.get(name).ok_or_else(|| {
            anyhow::anyhow!(
                "Domain profile '{}' not found in {}",
                name,
                root.join("mens/config/domain-profiles.yaml").display()
            )
        })?;

        let def = file.defaults.unwrap_or_else(|| DomainProfileDefaults {
            min_rating: None,
            ce_last_k: None,
            seq_len: None,
            validation_split_ratio: None,
            weight_decay: None,
            max_grad_norm: None,
            curriculum: None,
            trajectory_weighting: None,
            curriculum_schedule: None,
            chatml: None,
            reward_hook: None,
        });

        // Merge curriculum schedule
        let cur_sched = match (&profile.curriculum_schedule, &def.curriculum_schedule) {
            (Some(p), Some(d)) => Some(CurriculumSchedule {
                epoch_1_max_difficulty: p.epoch_1_max_difficulty.or(d.epoch_1_max_difficulty),
                epoch_2_max_difficulty: p.epoch_2_max_difficulty.or(d.epoch_2_max_difficulty),
                epoch_3_max_difficulty: p.epoch_3_max_difficulty.or(d.epoch_3_max_difficulty),
                curriculum_phases: p
                    .curriculum_phases
                    .clone()
                    .or_else(|| d.curriculum_phases.clone()),
            }),
            (Some(p), None) => Some(p.clone()),
            (None, Some(d)) => Some(d.clone()),
            (None, None) => None,
        };

        Ok(EffectiveDomainProfile {
            name: name.to_string(),
            description: profile.description.clone(),
            context_filter: profile.context_filter.clone(),
            mix_config: profile.mix_config.as_ref().map(|p| root.join(p)),
            system_prompt: profile.system_prompt.as_ref().map(|p| root.join(p)),
            base: profile.base.clone(),
            eval_gate: profile.eval_gate.as_ref().map(|p| root.join(p)),
            router: profile.router.clone(),

            min_rating: profile.min_rating.or(def.min_rating),
            ce_last_k: profile.ce_last_k.or(def.ce_last_k),
            seq_len: profile.seq_len.or(def.seq_len),
            validation_split_ratio: def.validation_split_ratio,
            weight_decay: def.weight_decay,
            max_grad_norm: profile.max_grad_norm.or(def.max_grad_norm),
            curriculum: def.curriculum,
            trajectory_weighting: profile.trajectory_weighting.or(def.trajectory_weighting),
            trajectory_tool_trace_boost: profile.trajectory_tool_trace_boost,
            curriculum_schedule: cur_sched,
            chatml: profile
                .chatml
                .clone()
                .or_else(|| def.chatml.clone())
                .unwrap_or_default(),
            reward_hook: profile
                .reward_hook
                .clone()
                .or_else(|| def.reward_hook.clone()),
        })
    }
}

#[cfg(test)]
mod spoke_base_tests {
    use super::*;

    #[test]
    fn domain_profile_deserializes_base_block() {
        let yaml = r#"
description: "test"
base:
  model: qwen2_5_coder_7b
  method: qlora
  preset: qwen_4080_16g
"#;
        let p: DomainProfile = serde_yaml::from_str(yaml).expect("parse");
        let base = p.base.expect("base present");
        assert_eq!(base.model, "qwen2_5_coder_7b");
        assert_eq!(base.method, TrainMethod::Qlora);
        assert_eq!(base.preset.as_deref(), Some("qwen_4080_16g"));
    }

    #[test]
    fn base_is_optional_for_backward_compat() {
        let yaml = r#"description: "legacy profile, no base""#;
        let p: DomainProfile = serde_yaml::from_str(yaml).expect("parse");
        assert!(p.base.is_none());
    }

    #[test]
    fn effective_profile_carries_base_through() {
        // load_domain_profile reads mens/config/domain-profiles.yaml from the
        // workspace root; this test runs from the crate dir, so point it up.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf();
        let eff = EffectiveDomainProfile::load_domain_profile("vox-lang", Some(&root))
            .expect("vox-lang profile loads");
        // vox-lang gains a base block in Task 1.5; until then this asserts the
        // field exists and is plumbed (None is acceptable pre-1.5).
        let _ = &eff.base;
        let _ = &eff.eval_gate;
        let _ = &eff.router;
    }

    #[test]
    fn list_profiles_returns_known_spokes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf();
        let file = DomainProfilesFile::load(Some(&root)).expect("load file");
        assert!(file.profiles.contains_key("vox-lang"));
    }

    // NOTE: agents_profile_has_prefer_local removed — the 'agents' profile is
    // retired by B0.1 and replaced by 'tool-selection' + 'argument-generation'.

    #[test]
    fn v1_finetuned_spoke_set_is_exactly_four() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf();
        let file = DomainProfilesFile::load(Some(&root)).expect("load file");
        let finetuned: Vec<&str> = file
            .profiles
            .iter()
            .filter(|(_, p)| {
                p.base
                    .as_ref()
                    .map_or(false, |b| b.method == TrainMethod::Qlora)
            })
            .map(|(k, _)| k.as_str())
            .collect();
        // The v1 fine-tuned set is exactly these 4 spokes
        let mut sorted = finetuned.clone();
        sorted.sort();
        assert_eq!(
            sorted,
            vec!["argument-generation", "rust", "tool-selection", "vox-lang"],
            "fine-tuned set mismatch: {:?}",
            sorted
        );
    }

    #[test]
    fn harness_profile_exists_without_base() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf();
        let file = DomainProfilesFile::load(Some(&root)).expect("load file");
        let harness = file
            .profiles
            .get("harness")
            .expect("harness profile must exist");
        assert!(
            harness.base.is_none(),
            "harness is a union profile — no base/training of its own"
        );
    }

    #[test]
    fn retired_spokes_have_no_base() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf();
        let file = DomainProfilesFile::load(Some(&root)).expect("load file");
        for retired in &["rocks", "research", "populi-meta", "research-expert"] {
            if let Some(p) = file.profiles.get(*retired) {
                assert!(
                    p.base.is_none(),
                    "retired spoke '{}' must not have a base",
                    retired
                );
            }
            // Absent is also fine (profile may be removed entirely)
        }
    }

    #[test]
    fn rust_review_lane_routes_to_no_adapter() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf();
        let file = DomainProfilesFile::load(Some(&root)).expect("load file");
        // No profile that has a base should trigger on lane:vox_rust_review
        for (name, profile) in &file.profiles {
            if profile.base.is_some() {
                let triggers = profile
                    .router
                    .as_ref()
                    .map(|r| r.triggers.as_slice())
                    .unwrap_or(&[]);
                assert!(
                    !triggers.iter().any(|t| t == "lane:vox_rust_review"),
                    "spoke '{}' has a base/adapter but also claims lane:vox_rust_review (review must route to base only)",
                    name
                );
            }
        }
    }

    #[test]
    fn spoke_router_prefer_local_defaults_false() {
        let yaml = r#"description: "no router""#;
        let p: DomainProfile = serde_yaml::from_str(yaml).expect("parse");
        assert!(p.router.is_none());

        let yaml2 = r#"
description: "router without prefer_local"
router:
  triggers: ["lane:test"]
  priority: 1
"#;
        let p2: DomainProfile = serde_yaml::from_str(yaml2).expect("parse");
        let r = p2.router.unwrap();
        assert!(!r.prefer_local, "prefer_local defaults to false");
    }
}
