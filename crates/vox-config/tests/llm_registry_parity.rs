//! Parity: every **Band-A** LLM/AI key that `vox-config` exposes an accessor for must
//! be registered in the `vox-llm-config` SSOT. Adding an accessor without registering it
//! fails here. Band-B (orchestrator routing/capability) accessors are listed as
//! explicitly deferred so this test does not silently pull Band-B scope forward.
use std::collections::HashSet;
use vox_config::vox_llm_config::LLM_CONFIG_KEYS;

/// Band-A env names that `vox-config` accessors read (inference.rs + endpoint consts).
const BAND_A_ACCESSOR_KEYS: &[&str] = &[
    "OPENROUTER_BASE_URL",
    "VOX_OPENAI_BASE_URL",
    "POPULI_URL",
    "OLLAMA_URL",
    "OLLAMA_TUNING_TEMPERATURE",
    "OLLAMA_TUNING_TOP_P",
    "OLLAMA_TUNING_NUM_CTX",
    "OPENAI_TUNING_TEMPERATURE",
    "OPENAI_TUNING_TOP_P",
    "ANTHROPIC_TUNING_TEMPERATURE",
    "ANTHROPIC_TUNING_TOP_P",
    "GEMINI_TUNING_TEMPERATURE",
    "GEMINI_TUNING_TOP_P",
    "TOGETHER_TUNING_TEMPERATURE",
    "TOGETHER_TUNING_TOP_P",
    "OPENROUTER_API_KEY",
    "HF_TOKEN",
];

/// Band-B accessors that live in vox-config today but are deferred to the Band-B plan.
/// They must NOT be required in the registry yet (documented exclusion, not an omission).
const DEFERRED_BAND_B_ACCESSOR_KEYS: &[&str] = &[
    "VOX_AUTO_ROUTING_PRIORITY",
    "VOX_GEMINI_ROUTE_POLICY",
    "GEMINI_DIRECT_MODEL",
    "OPENROUTER_GEMINI_MODEL",
];

#[test]
fn registry_covers_every_band_a_accessor() {
    let registered: HashSet<&str> = LLM_CONFIG_KEYS.iter().map(|k| k.env).collect();
    let missing: Vec<&str> = BAND_A_ACCESSOR_KEYS
        .iter()
        .copied()
        .filter(|k| !registered.contains(k))
        .collect();
    assert!(missing.is_empty(), "Band-A accessors not in registry: {missing:?}");
}

#[test]
fn deferred_band_b_keys_are_not_yet_registered() {
    let registered: HashSet<&str> = LLM_CONFIG_KEYS.iter().map(|k| k.env).collect();
    let leaked: Vec<&str> = DEFERRED_BAND_B_ACCESSOR_KEYS
        .iter()
        .copied()
        .filter(|k| registered.contains(k))
        .collect();
    assert!(
        leaked.is_empty(),
        "Band-B keys registered prematurely (move to Band-A list if intended): {leaked:?}"
    );
}

#[test]
fn registry_has_no_band_a_and_band_b_overlap() {
    let band_a: HashSet<&str> = BAND_A_ACCESSOR_KEYS.iter().copied().collect();
    for k in DEFERRED_BAND_B_ACCESSOR_KEYS {
        assert!(!band_a.contains(k), "{k} listed as both Band-A and deferred Band-B");
    }
}
