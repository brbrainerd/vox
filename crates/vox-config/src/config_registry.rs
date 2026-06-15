//! `CONFIG_KEYS`: the general federated config registry. Seed it with the knobs
//! that already have constants + a clear home. Domains migrate their
//! `operator_registry` rows here over time (Phase 2A.3). The
//! `vox ci config-registry-parity` gate (Phase 2B) enforces env coverage.

use crate::config_key::{ConfigKey, ConfigKind, DefaultValue, Group, GuiSurface, Home, Status};
use crate::operator_registry::ConfigClass;

/// The general config registry. ADD a row here for every new operational knob.
pub const CONFIG_KEYS: &[ConfigKey] = &[
    ConfigKey {
        key: "VOX_WASM_SKILL_FUEL",
        kind: ConfigKind::Int,
        default: DefaultValue::Literal("1000000000"),
        bound: Some((1_000_000.0, 100_000_000_000.0)),
        group: Group::Runtime,
        class: ConfigClass::NodeLocal,
        home: Home::Env,
        gui: Some(GuiSurface {
            section: "Runtime & Sandbox",
            options: &[],
        }),
        secret: false,
        status: Status::Active,
        label: "WASM skill fuel",
        hint: "Wasmtime instruction budget for skill execution.",
    },
    ConfigKey {
        key: "VOX_GAMIFY_ECONOMY_PATH",
        kind: ConfigKind::Path,
        default: DefaultValue::Computed("embedded gamify economy contract"),
        bound: None,
        group: Group::Tuning,
        class: ConfigClass::NodeLocal,
        home: Home::Contract("contracts/gamify/economy.v1.yaml"),
        gui: None,
        secret: false,
        status: Status::Active,
        label: "Gamify economy contract override",
        hint: "Path to an override economy.v1.yaml; defaults to the embedded contract.",
    },
    ConfigKey {
        key: "VOX_CIRCUIT_BREAKER_CONTRACT",
        kind: ConfigKind::Path,
        default: DefaultValue::Computed("embedded circuit-breaker contract"),
        bound: None,
        group: Group::Orchestrator,
        class: ConfigClass::NodeLocal,
        home: Home::Contract("contracts/orchestration/circuit-breaker.v1.yaml"),
        gui: None,
        secret: false,
        status: Status::Active,
        label: "Circuit-breaker contract override",
        hint: "Path to an override circuit-breaker.v1.yaml; defaults to the embedded contract.",
    },
];

/// All registered keys (for the parity gate).
pub fn registered_keys() -> impl Iterator<Item = &'static str> {
    CONFIG_KEYS.iter().map(|k| k.key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for k in CONFIG_KEYS {
            assert!(seen.insert(k.key), "duplicate ConfigKey: {}", k.key);
        }
    }

    #[test]
    fn env_homed_keys_are_vox_prefixed() {
        for k in CONFIG_KEYS {
            if matches!(k.home, Home::Env) {
                assert!(
                    k.key.starts_with("VOX_"),
                    "env knob must be VOX_*: {}",
                    k.key
                );
            }
        }
    }
}
