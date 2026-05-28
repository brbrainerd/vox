use std::env;
use std::fs;
use std::path::Path;

#[derive(serde::Deserialize)]
struct RoutingYaml {
    tiers: Vec<String>,
    strengths: Vec<String>,
    task_categories: Vec<String>,
    strength_inference: Vec<serde_yaml::Value>,
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let yaml_path =
        Path::new(&manifest_dir).join("../../contracts/orchestration/model-routing.v1.yaml");

    println!("cargo:rerun-if-changed={}", yaml_path.display());

    let yaml_content =
        fs::read_to_string(&yaml_path).expect("Failed to read model-routing.v1.yaml");
    let config: RoutingYaml = serde_yaml::from_str(&yaml_content).expect("Failed to parse YAML");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated.rs");

    let mut out = String::new();
    out.push_str("// AUTO-GENERATED from contracts/orchestration/model-routing.v1.yaml\n");
    out.push_str("// DO NOT EDIT DIRECTLY.\n\n");

    // Generate ModelTier
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize, Default)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("pub enum ModelTier {\n");
    for tier in &config.tiers {
        if tier == "Unknown" {
            out.push_str("    #[default]\n");
        }
        out.push_str(&format!("    {},\n", tier));
    }
    out.push_str("}\n\n");

    out.push_str("impl std::fmt::Display for ModelTier {\n");
    out.push_str("    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n");
    out.push_str("        match self {\n");
    for tier in &config.tiers {
        out.push_str(&format!(
            "            Self::{} => write!(f, \"{}\"),\n",
            tier,
            tier.to_lowercase()
        ));
    }
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    // Generate StrengthTag
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize, Default)]\n");
    out.push_str("#[serde(rename_all = \"snake_case\")]\n");
    out.push_str("pub enum StrengthTag {\n");
    out.push_str("    #[default]\n");
    out.push_str("    Unknown,\n");
    let mut strength_variants = Vec::new();
    for strength in &config.strengths {
        let pascal = strength
            .split(['-', '_'])
            .map(|s| {
                let mut c = s.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect::<String>();
        out.push_str(&format!("    {},\n", pascal));
        strength_variants.push((pascal, strength.clone()));
    }
    out.push_str("}\n\n");

    out.push_str("impl std::fmt::Display for StrengthTag {\n");
    out.push_str("    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n");
    out.push_str("        match self {\n");
    out.push_str("            Self::Unknown => write!(f, \"unknown\"),\n");
    for (pascal, raw) in &strength_variants {
        out.push_str(&format!(
            "            Self::{} => write!(f, \"{}\"),\n",
            pascal, raw
        ));
    }
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str("impl std::str::FromStr for StrengthTag {\n");
    out.push_str("    type Err = ();\n");
    out.push_str("    fn from_str(s: &str) -> Result<Self, Self::Err> {\n");
    out.push_str("        match s {\n");
    for (pascal, raw) in &strength_variants {
        out.push_str(&format!(
            "            \"{}\" => Ok(Self::{}),\n",
            raw, pascal
        ));
    }
    out.push_str("            _ => Ok(Self::Unknown),\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    // Generate TaskCategory
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize, Default)]\n");
    out.push_str("pub enum TaskCategory {\n");
    for cat in &config.task_categories {
        if cat == "General" {
            out.push_str("    #[default]\n");
        }
        out.push_str(&format!("    {},\n", cat));
    }
    out.push_str("}\n\n");

    out.push_str("impl std::fmt::Display for TaskCategory {\n");
    out.push_str("    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n");
    out.push_str("        match self {\n");
    for cat in &config.task_categories {
        out.push_str(&format!(
            "            Self::{} => write!(f, \"{}\"),\n",
            cat, cat
        ));
    }
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str("impl std::str::FromStr for TaskCategory {\n");
    out.push_str("    type Err = ();\n");
    out.push_str("    fn from_str(s: &str) -> Result<Self, Self::Err> {\n");
    out.push_str("        match s.to_lowercase().as_str() {\n");
    for cat in &config.task_categories {
        out.push_str(&format!(
            "            \"{}\" => Ok(Self::{}),\n",
            cat.to_lowercase(),
            cat
        ));
    }
    out.push_str("            _ => Ok(Self::General),\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    // Generate infer_strengths()
    out.push_str("#[must_use]\n");
    out.push_str("pub fn infer_strengths(id: &str, description: Option<&str>, supported_parameters: &[String]) -> Vec<StrengthTag> {\n");
    out.push_str("    let mut strengths = std::collections::BTreeSet::new();\n");
    out.push_str("    let provider_prefix = id.split('/').next().unwrap_or(\"\");\n");
    out.push_str("    let mut haystack = id.to_ascii_lowercase();\n");
    out.push_str("    if let Some(desc) = description {\n");
    out.push_str("        haystack.push(' ');\n");
    out.push_str("        haystack.push_str(&desc.to_ascii_lowercase());\n");
    out.push_str("    }\n\n");

    // Parse the yaml loosely using serde_yaml::Value
    let empty_vec = vec![];
    for block in &config.strength_inference {
        if let Some(mapping) = block.as_mapping() {
            let type_str = mapping
                .get(serde_yaml::Value::String("type".to_string()))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let rules = mapping
                .get(serde_yaml::Value::String("rules".to_string()))
                .and_then(|v| v.as_sequence())
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            match type_str {
                "parameter_graph" => {
                    for rule in rules {
                        let params = rule
                            .get("parameters")
                            .and_then(|v| v.as_sequence())
                            .unwrap_or(&empty_vec);
                        let strengths_to_add = rule
                            .get("strengths")
                            .and_then(|v| v.as_sequence())
                            .unwrap_or(&empty_vec);
                        out.push_str("    let has_param = ");
                        let mut param_checks = Vec::new();
                        for p in params {
                            if let Some(p_str) = p.as_str() {
                                param_checks.push(format!(
                                    "supported_parameters.iter().any(|p| p == \"{}\")",
                                    p_str
                                ));
                            }
                        }
                        if param_checks.is_empty() {
                            out.push_str("false;\n");
                        } else {
                            out.push_str(&param_checks.join(" || "));
                            out.push_str(";\n");
                            out.push_str("    if has_param {\n");
                            for s in strengths_to_add {
                                if let Some(s_str) = s.as_str()
                                    && let Some((pascal, _)) =
                                        strength_variants.iter().find(|(_, raw)| raw == s_str)
                                {
                                    out.push_str(&format!(
                                        "        strengths.insert(StrengthTag::{});\n",
                                        pascal
                                    ));
                                }
                            }
                            out.push_str("    }\n");
                        }
                    }
                }
                "provider_family" => {
                    out.push_str("    match provider_prefix {\n");
                    for rule in rules {
                        if let Some(match_str) = rule.get("match").and_then(|v| v.as_str()) {
                            let strengths_to_add = rule
                                .get("strengths")
                                .and_then(|v| v.as_sequence())
                                .unwrap_or(&empty_vec);
                            out.push_str(&format!("        \"{}\" => {{\n", match_str));
                            for s in strengths_to_add {
                                if let Some(s_str) = s.as_str()
                                    && let Some((pascal, _)) =
                                        strength_variants.iter().find(|(_, raw)| raw == s_str)
                                {
                                    out.push_str(&format!(
                                        "            strengths.insert(StrengthTag::{});\n",
                                        pascal
                                    ));
                                }
                            }
                            out.push_str("        }\n");
                        }
                    }
                    out.push_str("        _ => {}\n");
                    out.push_str("    }\n");
                }
                "name_regex" => {
                    out.push_str("    if strengths.is_empty() {\n");
                    for rule in rules {
                        if let Some(pattern) = rule.get("pattern").and_then(|v| v.as_str()) {
                            let strengths_to_add = rule
                                .get("strengths")
                                .and_then(|v| v.as_sequence())
                                .unwrap_or(&empty_vec);
                            let inner = pattern.replace(['(', ')'], "");
                            let parts: Vec<&str> = inner.split('|').collect();
                            out.push_str("        if ");
                            let checks: Vec<String> = parts
                                .iter()
                                .map(|p| format!("haystack.contains(\"{}\")", p))
                                .collect();
                            out.push_str(&checks.join(" || "));
                            out.push_str(" {\n");
                            for s in strengths_to_add {
                                if let Some(s_str) = s.as_str()
                                    && let Some((pascal, _)) =
                                        strength_variants.iter().find(|(_, raw)| raw == s_str)
                                {
                                    out.push_str(&format!(
                                        "            strengths.insert(StrengthTag::{});\n",
                                        pascal
                                    ));
                                }
                            }
                            out.push_str("        }\n");
                        }
                    }
                    out.push_str("    }\n");
                }
                "default" => {
                    out.push_str("    if strengths.is_empty() {\n");
                    for rule in rules {
                        let strengths_to_add = rule
                            .get("strengths")
                            .and_then(|v| v.as_sequence())
                            .unwrap_or(&empty_vec);
                        for s in strengths_to_add {
                            if let Some(s_str) = s.as_str()
                                && let Some((pascal, _)) =
                                    strength_variants.iter().find(|(_, raw)| raw == s_str)
                            {
                                out.push_str(&format!(
                                    "        strengths.insert(StrengthTag::{});\n",
                                    pascal
                                ));
                            }
                        }
                    }
                    out.push_str("    }\n");
                }
                _ => {}
            }
        }
    }

    out.push_str("    strengths.into_iter().collect()\n");
    out.push_str("}\n");

    append_capability_intent_codegen(&mut out, &yaml_content);

    fs::write(&dest_path, out).unwrap();
}

fn append_capability_intent_codegen(out: &mut String, yaml_content: &str) {
    use serde_yaml::Value;
    use std::collections::BTreeSet;

    let root: Value =
        serde_yaml::from_str(yaml_content).expect("parse yaml for capability codegen");
    let cap_inf = root
        .get("capability_inference")
        .expect("capability_inference missing");
    let params_rules = cap_inf
        .get("parameters_to_capabilities")
        .and_then(|v| v.as_sequence())
        .expect("parameters_to_capabilities");
    let mod_rules = cap_inf
        .get("modalities_to_capabilities")
        .and_then(|v| v.as_sequence())
        .expect("modalities_to_capabilities");

    let mut cap_names: BTreeSet<String> = BTreeSet::new();
    for rule in params_rules {
        if let Some(c) = rule.get("capability").and_then(|v| v.as_str()) {
            cap_names.insert(c.to_string());
        }
    }
    for rule in mod_rules {
        if let Some(c) = rule.get("capability").and_then(|v| v.as_str()) {
            cap_names.insert(c.to_string());
        }
    }

    out.push_str("use regex::Regex;\n");
    out.push_str("use std::sync::OnceLock;\n\n");

    // --- Capability ---
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]\n");
    out.push_str("pub enum Capability {\n");
    for c in &cap_names {
        out.push_str(&format!("    {},\n", snake_to_pascal_upper(c)));
    }
    out.push_str("}\n\n");

    out.push_str("#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]\n");
    out.push_str("pub struct CapabilityFlags {\n");
    for c in &cap_names {
        out.push_str(&format!("    #[serde(default)]\n    pub {}: bool,\n", c));
    }
    out.push_str("}\n\n");

    out.push_str("#[must_use]\n");
    out.push_str("pub fn infer_capabilities(\n");
    out.push_str("    supported_parameters: &[String],\n");
    out.push_str("    input_modalities: &[String],\n");
    out.push_str("    output_modalities: &[String],\n");
    out.push_str(") -> CapabilityFlags {\n");
    out.push_str("    let mut out = CapabilityFlags::default();\n");
    out.push_str("    let has_param = |p: &str| {\n");
    out.push_str("        supported_parameters.iter().any(|x| x.eq_ignore_ascii_case(p))\n");
    out.push_str("    };\n");
    out.push_str("    let has_in_mod = |m: &str| {\n");
    out.push_str("        input_modalities.iter().any(|x| x.eq_ignore_ascii_case(m))\n");
    out.push_str("    };\n");
    out.push_str("    let has_out_mod = |m: &str| {\n");
    out.push_str("        output_modalities.iter().any(|x| x.eq_ignore_ascii_case(m))\n");
    out.push_str("    };\n");
    for rule in params_rules {
        let Some(cap) = rule.get("capability").and_then(|v| v.as_str()) else {
            continue;
        };
        let params = rule
            .get("parameters")
            .and_then(|v| v.as_sequence())
            .map(|s| {
                s.iter()
                    .filter_map(|v| v.as_str())
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if params.is_empty() {
            continue;
        }
        let checks = params
            .iter()
            .map(|p| {
                format!(
                    "has_param(\"{}\")",
                    p.replace('\\', "\\\\").replace('"', "\\\"")
                )
            })
            .collect::<Vec<_>>()
            .join(" || ");
        out.push_str(&format!(
            "    if {} {{\n        out.{} = true;\n    }}\n",
            checks, cap
        ));
    }
    for rule in mod_rules {
        let Some(cap) = rule.get("capability").and_then(|v| v.as_str()) else {
            continue;
        };
        if let Some(im) = rule.get("input_modality").and_then(|v| v.as_str()) {
            out.push_str(&format!(
                "    if has_in_mod(\"{}\") {{ out.{} = true; }}\n",
                im.replace('\\', "\\\\").replace('"', "\\\""),
                cap
            ));
        }
        if let Some(om) = rule.get("output_modality").and_then(|v| v.as_str()) {
            out.push_str(&format!(
                "    if has_out_mod(\"{}\") {{ out.{} = true; }}\n",
                om.replace('\\', "\\\\").replace('"', "\\\""),
                cap
            ));
        }
    }
    out.push_str("    out\n");
    out.push_str("}\n\n");

    // --- Intent ---
    let intent_map = root
        .get("intent_to_required_capabilities")
        .and_then(|v| v.as_mapping())
        .expect("intent_to_required_capabilities");
    let mut intent_keys: Vec<String> = intent_map
        .keys()
        .filter_map(|k| k.as_str().map(str::to_string))
        .collect();
    intent_keys.sort();

    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]\n");
    out.push_str("pub enum PromptIntent {\n");
    for k in &intent_keys {
        out.push_str(&format!("    {},\n", snake_to_pascal_upper(k)));
    }
    out.push_str("}\n\n");

    out.push_str("#[must_use]\n");
    out.push_str(
        "pub fn intent_required_capabilities(intent: PromptIntent) -> &'static [Capability] {\n",
    );
    out.push_str("    match intent {\n");
    for k_str in &intent_keys {
        let Some(val) = intent_map.get(Value::String(k_str.clone())) else {
            continue;
        };
        let caps_seq = val
            .as_sequence()
            .map(|s| {
                s.iter()
                    .filter_map(|v| v.as_str())
                    .map(snake_to_pascal_upper)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let arms = caps_seq
            .iter()
            .map(|c| format!("Capability::{}", c))
            .collect::<Vec<_>>()
            .join(", ");
        let variant = snake_to_pascal_upper(k_str);
        if caps_seq.is_empty() {
            out.push_str(&format!("        PromptIntent::{} => &[],\n", variant));
        } else {
            out.push_str(&format!(
                "        PromptIntent::{} => &[{}],\n",
                variant, arms
            ));
        }
    }
    out.push_str("    }\n");
    out.push_str("}\n\n");

    // prompt_intent_inference regex (case-insensitive)
    let prompt_blocks = root
        .get("prompt_intent_inference")
        .and_then(|v| v.as_sequence())
        .cloned()
        .unwrap_or_default();
    let mut regex_rows: Vec<(String, Vec<String>)> = Vec::new();
    for block in &prompt_blocks {
        let Some(mapping) = block.as_mapping() else {
            continue;
        };
        let ty = mapping
            .get(Value::String("kind".into()))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if ty != "name_regex" {
            continue;
        }
        let Some(rules) = mapping
            .get(Value::String("rules".into()))
            .and_then(|v| v.as_sequence())
        else {
            continue;
        };
        for rule in rules {
            let Some(pat) = rule.get("pattern").and_then(|v| v.as_str()) else {
                continue;
            };
            let intents: Vec<String> = rule
                .get("intents")
                .and_then(|v| v.as_sequence())
                .map(|s| {
                    s.iter()
                        .filter_map(|v| v.as_str())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            if intents.is_empty() {
                continue;
            }
            regex_rows.push((pat.to_string(), intents));
        }
    }

    out.push_str("static PROMPT_INTENT_RULES: OnceLock<Vec<(Regex, Vec<PromptIntent>)>> = OnceLock::new();\n\n");
    out.push_str("fn prompt_intent_regex_rules() -> &'static Vec<(Regex, Vec<PromptIntent>)> {\n");
    out.push_str("    PROMPT_INTENT_RULES.get_or_init(|| {\n");
    out.push_str("        vec![\n");
    for (pat, intents) in &regex_rows {
        let intent_variants: Vec<String> = intents
            .iter()
            .map(|i| format!("PromptIntent::{}", snake_to_pascal_upper(i)))
            .collect();
        let pat_clean = pat.replace('\\', "\\\\");
        out.push_str(&format!(
            "            (Regex::new(r#\"(?i){}\"#).expect(\"prompt intent regex\"), vec![{}]),\n",
            pat_clean,
            intent_variants.join(", ")
        ));
    }
    out.push_str("        ]\n");
    out.push_str("    })\n");
    out.push_str("}\n\n");

    out.push_str("#[must_use]\n");
    out.push_str("pub fn infer_prompt_intents(prompt: &str) -> Vec<PromptIntent> {\n");
    out.push_str("    let pl = prompt.to_ascii_lowercase();\n");
    out.push_str("    let mut out = Vec::new();\n");
    out.push_str("    for (rx, intents) in prompt_intent_regex_rules().iter() {\n");
    out.push_str("        if rx.is_match(&pl) {\n");
    out.push_str("            for i in intents {\n");
    out.push_str("                if !out.contains(i) {\n");
    out.push_str("                    out.push(*i);\n");
    out.push_str("                }\n");
    out.push_str("            }\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    out\n");
    out.push_str("}\n");
}

fn snake_to_pascal_upper(s: &str) -> String {
    s.split(['_', '.'])
        .filter(|p| !p.is_empty())
        .map(|w| {
            let mut ch = w.chars();
            match ch.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
            }
        })
        .collect()
}
