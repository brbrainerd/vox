//! Token registry: loads vox.tokens.json and validates token references at compile time.
//!
//! Token keys are stored in CSS-var style (e.g. `"color-primary"`) matching the suffix
//! of the `--vox-*` custom properties that `lower.rs` emits. A `TokenRef("vox-color-primary")`
//! in the Web IR maps to registry key `"color-primary"` after stripping the `"vox-"` prefix.

pub mod contrast;

use std::collections::HashMap;

pub use contrast::{ContrastPair, TextRole, wcag21_contrast_ratio};

/// A named surface pair: foreground and background token keys.
#[derive(Debug, Clone)]
pub struct SurfacePairEntry {
    /// CSS-var key for the foreground (e.g. `"color-background"`).
    pub fg_key: String,
    /// CSS-var key for the background (e.g. `"color-primary"`).
    pub bg_key: String,
}

/// Registry of design tokens loaded from `vox.tokens.json`.
///
/// Build with [`TokenRegistry::load_from_str`] or [`TokenRegistry::load_from_project_dir`].
#[derive(Debug, Default, Clone)]
pub struct TokenRegistry {
    /// CSS-var-style key (e.g. `"color-primary"`) → hex or CSS value string.
    pub by_css_var: HashMap<String, String>,
    /// Contrast pairs declared via the `on` + `text_role` metadata fields.
    pub contrast_pairs: Vec<ContrastPair>,
    /// Named surface pairs (name → fg/bg token keys). Populated from `surface.*` entries
    /// with `$surface_pair: true` in `vox.tokens.json`.
    pub surface_pairs: HashMap<String, SurfacePairEntry>,
}

impl TokenRegistry {
    /// Build a registry from a parsed JSON value (the root of `vox.tokens.json`).
    pub fn load_from_value(root: &serde_json::Value) -> Self {
        let mut registry = TokenRegistry::default();
        if let Some(obj) = root.as_object() {
            for (key, val) in obj {
                if key.starts_with('$') {
                    continue;
                }
                if key == "surface" {
                    walk_surface_pairs(val, &mut registry);
                } else {
                    walk_json(val, &[key.as_str()], &mut registry);
                }
            }
        }
        registry
    }

    /// Look up a named surface pair (e.g. `"primary"` → fg/bg token keys).
    pub fn lookup_surface(&self, name: &str) -> Option<&SurfacePairEntry> {
        self.surface_pairs.get(name)
    }

    /// Parse `vox.tokens.json` content and build a registry.
    pub fn load_from_str(json: &str) -> Result<Self, serde_json::Error> {
        let val: serde_json::Value = serde_json::from_str(json)?;
        Ok(Self::load_from_value(&val))
    }

    /// Load `<project_dir>/vox.tokens.json` if it exists. Returns `None` if the file is
    /// absent or unparseable (callers treat absence as "no registry" rather than an error).
    pub fn load_from_project_dir(project_dir: &std::path::Path) -> Option<Self> {
        let content = std::fs::read_to_string(project_dir.join("vox.tokens.json")).ok()?;
        Self::load_from_str(&content).ok()
    }

    /// Look up a token by CSS-var key (e.g. `"color-primary"`).
    pub fn lookup(&self, css_var_key: &str) -> Option<&str> {
        self.by_css_var.get(css_var_key).map(|s| s.as_str())
    }

    /// All registered CSS-var keys in an unspecified order.
    pub fn all_keys(&self) -> impl Iterator<Item = &str> {
        self.by_css_var.keys().map(|s| s.as_str())
    }

    /// Validate all declared contrast pairs and return any WCAG diagnostics.
    ///
    /// Each item is `(foreground_key, background_key, text_role, ratio, severity)` where
    /// `severity` is `"warning"` or `"error"`.
    pub fn validate_contrast(&self) -> Vec<ContrastDiagnostic> {
        let mut out = Vec::new();
        for pair in &self.contrast_pairs {
            let Some(fg_hex) = self.by_css_var.get(&pair.foreground_key) else {
                continue;
            };
            let Some(bg_hex) = self.by_css_var.get(&pair.background_key) else {
                continue;
            };
            let Some(ratio) = wcag21_contrast_ratio(fg_hex, bg_hex) else {
                continue;
            };

            let error_threshold = pair.text_role.error_threshold();
            let warn_threshold = pair.text_role.warn_threshold();

            if ratio < error_threshold {
                out.push(ContrastDiagnostic {
                    foreground_key: pair.foreground_key.clone(),
                    background_key: pair.background_key.clone(),
                    ratio,
                    threshold: error_threshold,
                    severity: ContrastSeverity::Error,
                });
            } else if ratio < warn_threshold {
                out.push(ContrastDiagnostic {
                    foreground_key: pair.foreground_key.clone(),
                    background_key: pair.background_key.clone(),
                    ratio,
                    threshold: warn_threshold,
                    severity: ContrastSeverity::Warning,
                });
            }
        }
        out
    }
}

/// A WCAG contrast diagnostic produced by [`TokenRegistry::validate_contrast`].
#[derive(Debug, Clone)]
pub struct ContrastDiagnostic {
    pub foreground_key: String,
    pub background_key: String,
    pub ratio: f64,
    pub threshold: f64,
    pub severity: ContrastSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContrastSeverity {
    Warning,
    Error,
}

/// Parse `surface.*` entries with `$surface_pair: true` into registry.surface_pairs.
/// Token dot-paths (e.g. "color.text") are converted to CSS-var keys ("color-text").
fn walk_surface_pairs(surface_obj: &serde_json::Value, registry: &mut TokenRegistry) {
    let Some(obj) = surface_obj.as_object() else {
        return;
    };
    for (name, entry) in obj {
        if name.starts_with('$') {
            continue;
        }
        let Some(entry_obj) = entry.as_object() else {
            continue;
        };
        if !entry_obj
            .get("$surface_pair")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            continue;
        }
        let Some(fg_path) = entry_obj.get("fg").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(bg_path) = entry_obj.get("bg").and_then(|v| v.as_str()) else {
            continue;
        };
        registry.surface_pairs.insert(
            name.clone(),
            SurfacePairEntry {
                fg_key: fg_path.replace('.', "-"),
                bg_key: bg_path.replace('.', "-"),
            },
        );
    }
}

fn walk_json(val: &serde_json::Value, path: &[&str], registry: &mut TokenRegistry) {
    match val {
        serde_json::Value::String(s) => {
            registry.by_css_var.insert(path.join("-"), s.clone());
        }
        serde_json::Value::Object(obj) => {
            if let Some(value_str) = obj.get("value").and_then(|v| v.as_str()) {
                // Annotated color token: { "value": "#hex", "on": "color.background", ... }
                let key = path.join("-");
                registry
                    .by_css_var
                    .insert(key.clone(), value_str.to_string());

                if let Some(on_path) = obj.get("on").and_then(|v| v.as_str()) {
                    let bg_key = on_path.replace('.', "-");
                    let text_role = obj
                        .get("text_role")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<TextRole>().ok())
                        .unwrap_or(TextRole::Body);
                    registry.contrast_pairs.push(ContrastPair {
                        foreground_key: key,
                        background_key: bg_key,
                        text_role,
                    });
                }
            } else {
                // Sub-group: recurse into children
                for (subkey, subval) in obj {
                    if subkey.starts_with('$') {
                        continue;
                    }
                    let mut new_path = path.to_vec();
                    new_path.push(subkey.as_str());
                    walk_json(subval, &new_path, registry);
                }
            }
        }
        _ => {}
    }
}

/// Return up to 3 registry keys with Levenshtein distance ≤ 2 from `query`.
pub fn suggest_tokens<'a>(query: &str, registry: &'a TokenRegistry) -> Vec<&'a str> {
    let mut candidates: Vec<(&str, usize)> = registry
        .all_keys()
        .filter_map(|k| {
            let d = strsim::levenshtein(query, k);
            if d <= 2 { Some((k, d)) } else { None }
        })
        .collect();
    candidates.sort_by_key(|(_, d)| *d);
    candidates.truncate(3);
    candidates.into_iter().map(|(k, _)| k).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_TOKENS: &str = r##"{
        "color": {
            "primary": "#3a86ff",
            "background": "#ffffff",
            "text": {
                "value": "#1d3557",
                "on": "color.background",
                "text_role": "body"
            }
        },
        "spacing": { "md": "16px" }
    }"##;

    #[test]
    fn loads_flat_string_tokens() {
        let reg = TokenRegistry::load_from_str(MINIMAL_TOKENS).unwrap();
        assert_eq!(reg.lookup("color-primary"), Some("#3a86ff"));
        assert_eq!(reg.lookup("color-background"), Some("#ffffff"));
        assert_eq!(reg.lookup("spacing-md"), Some("16px"));
    }

    #[test]
    fn loads_annotated_token_value() {
        let reg = TokenRegistry::load_from_str(MINIMAL_TOKENS).unwrap();
        assert_eq!(reg.lookup("color-text"), Some("#1d3557"));
    }

    #[test]
    fn contrast_pair_registered() {
        let reg = TokenRegistry::load_from_str(MINIMAL_TOKENS).unwrap();
        assert_eq!(reg.contrast_pairs.len(), 1);
        assert_eq!(reg.contrast_pairs[0].foreground_key, "color-text");
        assert_eq!(reg.contrast_pairs[0].background_key, "color-background");
        assert_eq!(reg.contrast_pairs[0].text_role, TextRole::Body);
    }

    #[test]
    fn passing_contrast_produces_no_diagnostics() {
        let reg = TokenRegistry::load_from_str(MINIMAL_TOKENS).unwrap();
        let diags = reg.validate_contrast();
        assert!(
            diags.is_empty(),
            "unexpected contrast failures: {:?}",
            diags
        );
    }

    #[test]
    fn failing_contrast_produces_error() {
        let json = r##"{
            "color": {
                "background": "#ffffff",
                "faint": {
                    "value": "#cccccc",
                    "on": "color.background",
                    "text_role": "body"
                }
            }
        }"##;
        let reg = TokenRegistry::load_from_str(json).unwrap();
        let diags = reg.validate_contrast();
        assert!(
            !diags.is_empty(),
            "expected a contrast failure for #cccccc on #ffffff"
        );
        assert_eq!(diags[0].severity, ContrastSeverity::Error);
    }

    #[test]
    fn suggest_tokens_finds_close_match() {
        let reg = TokenRegistry::load_from_str(MINIMAL_TOKENS).unwrap();
        let suggestions = suggest_tokens("color-primaty", &reg);
        assert!(
            suggestions.contains(&"color-primary"),
            "expected 'color-primary' in suggestions, got {:?}",
            suggestions
        );
    }

    #[test]
    fn unknown_key_no_suggestions() {
        let reg = TokenRegistry::load_from_str(MINIMAL_TOKENS).unwrap();
        let suggestions = suggest_tokens("completely-different-xyz", &reg);
        assert!(suggestions.is_empty());
    }
}
