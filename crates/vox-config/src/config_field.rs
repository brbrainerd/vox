//! Catalog row for a single config knob — the GUI/introspection view of a
//! `ConfigKey` plus its *current* value. Produced by `#[derive(VoxConfig)]`.

use crate::config_key::ConfigKind;

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigField {
    pub key: &'static str,
    pub kind: ConfigKind,
    pub current: String,
    pub default: String,
    pub group: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_field_roundtrips() {
        let f = ConfigField {
            key: "VOX_X_FOO",
            kind: ConfigKind::Int,
            current: "5".into(),
            default: "3".into(),
            group: "General",
            label: "Foo",
            hint: "",
        };
        assert_eq!(f.current, "5");
        assert_ne!(f.current, f.default);
    }
}
