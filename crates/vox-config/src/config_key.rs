//! The single shared schema for every operational config knob (the federated
//! registry's SSOT *type*). `operator_registry` and the GUI `FIELDS`/`settingsIndex`
//! become VIEWS over `CONFIG_KEYS` (Phase 2A.2 / 2C). Protocol/crypto/grammar/
//! calibration constants are NOT config and never get a `ConfigKey`.

/// Value kind for a config knob (superset of the GUI `Kind` and the planned LLM kinds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigKind {
    Bool,
    Int,
    Float,
    String,
    Path,
    Url,
    Enum,
}

/// Where a knob's DEFAULT comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultValue {
    /// A literal default rendered as a string (MUST equal the in-code constant).
    Literal(&'static str),
    /// Computed at read-time by a named accessor (e.g. provider-derived URL).
    Computed(&'static str),
}

/// Coarse UI/topic grouping (federated — domains extend this deliberately).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    General,
    ModelsAndEndpoints,
    Tuning,
    Training,
    Orchestrator,
    Runtime,
    Storage,
    Mesh,
    Security,
    Telemetry,
}

/// Canonical home / value-SSOT pointer for the knob (the "ONE home" rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Home {
    Env,
    VoxToml,
    /// Value lives in a typed per-domain contract (federation), e.g.
    /// `Contract("contracts/scaling/policy.yaml")`.
    Contract(&'static str),
    Gui,
}

/// Lifecycle: `Active` = read in code today; `Declared` = registered, not yet wired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Active,
    Declared,
}

/// GUI surfacing directive. `None` on a `ConfigKey` = not surfaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiSurface {
    pub section: &'static str,
    /// Optional enum options for a dropdown.
    pub options: &'static [&'static str],
}

/// One operational config knob. Reuses `operator_registry::ConfigClass`.
#[derive(Debug, Clone, Copy)]
pub struct ConfigKey {
    /// The `VOX_*` env name or config key (the unique id).
    pub key: &'static str,
    pub kind: ConfigKind,
    pub default: DefaultValue,
    /// Numeric validation bound `(min, max)` — `None` for non-numeric/unbounded.
    pub bound: Option<(f64, f64)>,
    pub group: Group,
    pub class: crate::operator_registry::ConfigClass,
    pub home: Home,
    pub gui: Option<GuiSurface>,
    pub secret: bool,
    pub status: Status,
    pub label: &'static str,
    pub hint: &'static str,
}

impl ConfigKey {
    /// A numeric value is valid iff finite and within `bound` (if any).
    #[must_use]
    pub fn validate_numeric(&self, v: f64) -> bool {
        if !v.is_finite() {
            return false;
        }
        match self.bound {
            Some((lo, hi)) => v >= lo && v <= hi,
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operator_registry::ConfigClass;

    fn sample() -> ConfigKey {
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
            hint: "Wasmtime instruction budget",
        }
    }

    #[test]
    fn validate_numeric_respects_bounds() {
        let k = sample();
        assert!(k.validate_numeric(1_000_000_000.0));
        assert!(!k.validate_numeric(0.0)); // below min
        assert!(!k.validate_numeric(f64::NAN)); // not finite
        assert!(!k.validate_numeric(1e15)); // above max
    }
}
