//! Registry-derived command metadata for catalog / discoverability (single embedded YAML parse).
//!
//! The on-disk registry must match the embed (see `command-compliance`); dependents rebuild
//! whenever `contracts/cli/command-registry.yaml` changes via `include_str!`.

use std::collections::HashSet;

use crate::command_registry_model::RegistryOperation;

/// Embedded registry (must stay in sync with `contracts/cli/command-registry.yaml` on disk).
pub const EMBEDDED_COMMAND_REGISTRY_YAML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../contracts/cli/command-registry.yaml"
));

fn vox_cli_operations() -> &'static [RegistryOperation] {
    static OPS: std::sync::OnceLock<Vec<RegistryOperation>> = std::sync::OnceLock::new();
    OPS.get_or_init(|| {
        let reg: crate::command_registry_model::RegistryFile =
            serde_yaml::from_str(EMBEDDED_COMMAND_REGISTRY_YAML)
                .expect("embedded command-registry.yaml must parse");
        reg.operations
            .into_iter()
            .filter(|op| op.surface == "vox-cli")
            .collect()
    })
}

/// Merge `feature_gate` from all **`vox-cli`** registry rows sharing this path (stable dedupe).
pub fn merged_feature_gate_from_vox_cli_ops(
    vox_ops: &[RegistryOperation],
    path: &[String],
) -> Option<String> {
    let gates: Vec<String> = vox_ops
        .iter()
        .filter(|op| op.path.as_slice() == path)
        .filter_map(|op| op.feature_gate.clone())
        .collect();
    if gates.is_empty() {
        return None;
    }
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::new();
    for g in gates {
        if seen.insert(g.clone()) {
            out.push(g);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out.join("|"))
    }
}

fn ops_for_path(path: &[String]) -> Vec<&'static RegistryOperation> {
    vox_cli_operations()
        .iter()
        .filter(|op| op.path.as_slice() == path)
        .collect()
}

/// Merge `feature_gate` from all registry rows sharing this path (stable dedupe).
pub fn merged_feature_gate(path: &[String]) -> Option<String> {
    merged_feature_gate_from_vox_cli_ops(vox_cli_operations(), path)
}

fn latin_ns_to_catalog_group(ns: &str, path: &[String]) -> String {
    match ns {
        "fabrica" => "fabrica".to_string(),
        "mens" => "mens".to_string(),
        "diag" => "diag".to_string(),
        "ars" => "ars".to_string(),
        "recensio" => "recensio".to_string(),
        "ci" | "codex" | "dei" => "core".to_string(),
        _ => {
            let top = path.first().map(String::as_str).unwrap_or("core");
            if matches!(top, "oratio" | "speech") {
                "oratio".to_string()
            } else {
                "core".to_string()
            }
        }
    }
}

fn top_level_product_lane(path: &[String]) -> Option<&'static str> {
    match path.first().map(String::as_str) {
        Some(
            "build" | "check" | "test" | "run" | "dev" | "bundle" | "fmt" | "lsp" | "completions"
            | "fabrica",
        ) => Some("app"),
        Some("script" | "populi") => Some("workflow"),
        Some("mens" | "dei" | "review" | "recensio" | "oratio" | "speech" | "train" | "live") => {
            Some("ai")
        }
        Some("openclaw" | "skill" | "snippet" | "share" | "ars") => Some("interop"),
        Some("codex" | "db" | "scientia") => Some("data"),
        Some(
            "add" | "remove" | "update" | "lock" | "sync" | "upgrade" | "pm" | "ci" | "doctor"
            | "diag" | "architect" | "stub-check" | "secrets" | "clavis" | "login" | "logout"
            | "commands" | "shell" | "migrate",
        ) => Some("platform"),
        _ => None,
    }
}

fn product_lane_from_registry(path: &[String]) -> Option<String> {
    let hits = ops_for_path(path);
    if let Some(h) = hits.iter().find(|h| h.product_lane.is_some()) {
        return h.product_lane.clone();
    }

    if path.len() > 1 {
        let top_only = vec![path[0].clone()];
        let top_hits = ops_for_path(&top_only);
        if let Some(h) = top_hits.iter().find(|h| h.product_lane.is_some()) {
            return h.product_lane.clone();
        }
    }

    top_level_product_lane(path).map(str::to_string)
}

/// `source_group` for `vox commands` JSON/text (registry-first, then heuristic).
pub fn catalog_source_group(path: &[String]) -> String {
    if let Some(lane) = product_lane_from_registry(path) {
        return lane;
    }
    if let Some(g) = catalog_source_group_from_registry(path) {
        return g;
    }
    fallback_source_group(path)
}

fn catalog_source_group_from_registry(path: &[String]) -> Option<String> {
    let hits = ops_for_path(path);
    if hits.is_empty() {
        return None;
    }
    if let Some(h) = hits.iter().find(|h| h.catalog_group.is_some()) {
        return h.catalog_group.clone();
    }
    hits.first()
        .and_then(|h| h.latin_ns.as_deref())
        .map(|ns| latin_ns_to_catalog_group(ns, path))
}

/// When no registry row matches this exact path (e.g. `fabrica build` shim), infer lane from the first segment.
pub fn fallback_source_group(path: &[String]) -> String {
    let top = path.first().map(String::as_str).unwrap_or("unknown");
    if matches!(top, "fabrica" | "diag" | "ars" | "recensio") {
        return top.to_string();
    }
    if matches!(
        top,
        "build" | "check" | "test" | "run" | "dev" | "bundle" | "fmt" | "script" | "completions"
    ) {
        return "fabrica".to_string();
    }
    if matches!(top, "doctor" | "architect" | "stub-check") {
        return "diag".to_string();
    }
    if matches!(top, "snippet" | "share" | "skill" | "openclaw" | "ludus") {
        return "ars".to_string();
    }
    if top == "review" {
        return "recensio".to_string();
    }
    if matches!(top, "oratio" | "speech") {
        return "oratio".to_string();
    }
    if top == "migrate" {
        return "pm".to_string();
    }
    "core".to_string()
}
