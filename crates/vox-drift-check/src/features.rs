use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use vox_code_audit::rules::Language;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Loc {
    pub line: usize, // 1-indexed
    pub col: usize,  // 0-indexed
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LiteralContext {
    Code,
    Test,
    Doc,
    ConstDecl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UnitHint {
    Millis,
    Seconds,
    Bytes,
    Count,
    Bare,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiteralLoc {
    pub value: String,
    pub loc: Loc,
    pub ctx: LiteralContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumericLoc {
    pub value: f64,
    pub unit: Option<UnitHint>,
    pub loc: Loc,
    /// True when the literal is part of a `const` or `static` item's initializer.
    /// SSOT timeout/string consts deliberately spell the literal — rules skip these.
    #[serde(default)]
    pub in_const: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSite {
    pub path: Vec<String>,
    pub arity: u8,
    pub loc: Loc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodySignature {
    pub hash: u64,
    pub line_count: u32,
    pub parent_fn: Option<String>,
    pub loc: Loc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportLoc {
    pub path: Vec<String>,
    pub symbol: Option<String>,
    pub loc: Loc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FnDef {
    pub name: String,
    pub body_hash: u64,
    pub sig_hash: u64,
    pub loc: Loc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedFeatures {
    pub file: PathBuf,
    pub language: Language,
    pub crate_name: Option<String>,
    pub string_literals: Vec<LiteralLoc>,
    pub numeric_literals: Vec<NumericLoc>,
    pub call_sites: Vec<CallSite>,
    pub body_signatures: Vec<BodySignature>,
    pub imports: Vec<ImportLoc>,
    pub fn_definitions: Vec<FnDef>,
    /// Per-rule line allowlist parsed from `// drift-allow(rule-id[,rule-id…])`
    /// source comments. A rule that emits a finding at one of these lines must
    /// suppress it (see `ExtractedFeatures::is_allowed`).
    #[serde(default)]
    pub allowed_lines: std::collections::HashMap<String, std::collections::HashSet<usize>>,
}

impl ExtractedFeatures {
    pub fn new(file: PathBuf, language: Language) -> Self {
        Self {
            file,
            language,
            crate_name: None,
            string_literals: Vec::new(),
            numeric_literals: Vec::new(),
            call_sites: Vec::new(),
            body_signatures: Vec::new(),
            imports: Vec::new(),
            fn_definitions: Vec::new(),
            allowed_lines: std::collections::HashMap::new(),
        }
    }

    /// True when an in-source `// drift-allow(<rule_id>)` comment covers `line`.
    /// Annotations cover the line they sit on AND the following line, so both
    /// `Duration::from_secs(5); // drift-allow(timeout-literal): …` (trailing)
    /// and the comment-above style are honored.
    pub fn is_allowed(&self, rule_id: &str, line: usize) -> bool {
        self.allowed_lines
            .get(rule_id)
            .is_some_and(|set| set.contains(&line))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_loc_round_trips_serde() {
        let lit = LiteralLoc {
            value: "hello world".into(),
            loc: Loc { line: 5, col: 3 },
            ctx: LiteralContext::Code,
        };
        let json = serde_json::to_string(&lit).unwrap();
        let back: LiteralLoc = serde_json::from_str(&json).unwrap();
        assert_eq!(back.value, "hello world");
        assert_eq!(back.loc.line, 5);
        assert!(matches!(back.ctx, LiteralContext::Code));
    }

    #[test]
    fn numeric_loc_unit_hint_default() {
        let n = NumericLoc {
            value: 30.0,
            unit: None,
            loc: Loc::default(),
            in_const: false,
        };
        assert_eq!(n.value, 30.0);
        assert!(n.unit.is_none());
    }

    #[test]
    fn extracted_features_default_is_empty() {
        let f = ExtractedFeatures::new(std::path::PathBuf::from("foo.rs"), Language::Rust);
        assert!(f.string_literals.is_empty());
        assert!(f.call_sites.is_empty());
    }
}
