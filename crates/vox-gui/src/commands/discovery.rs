//! Tauri commands powering the Console discovery engine: candidate suggestion,
//! per-command help lookup, and exposure recording. Ranking/ledger logic lives in
//! `vox_gamify::discovery`; these commands adapt the command catalog to it.

use serde::Serialize;
use vox_cli::command_catalog::{CommandCatalogEntry, build_catalog};

/// A single suggestion returned to the UI.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Suggestion {
    /// Canonical action id, e.g. "vox.scientia.review".
    pub action_id: String,
    /// Full command line fragment to complete to, e.g. "scientia review".
    pub completion: String,
    pub about: String,
}

/// Build the canonical action id for a catalog entry: "vox" + dotted path.
pub fn action_id_for(entry: &CommandCatalogEntry) -> String {
    let mut parts = vec!["vox".to_string()];
    parts.extend(entry.path.iter().cloned());
    parts.join(".")
}

/// Filter the catalog to entries whose dotted path starts with the typed words.
/// `typed` is the text after the leading "vox " (may be empty). Runnable leaves
/// only (entries with no subcommands), capped at `limit`.
pub fn match_catalog(
    entries: &[CommandCatalogEntry],
    typed: &str,
    limit: usize,
) -> Vec<Suggestion> {
    let needle: Vec<&str> = typed.split_whitespace().collect();
    let mut out = Vec::new();
    for e in entries {
        if e.has_subcommands {
            continue;
        }
        let path_str = e.path.join(" ");
        let matches = needle.is_empty()
            || path_str.starts_with(&needle.join(" "))
            || e.aliases.iter().any(|a| a.starts_with(typed));
        if matches {
            out.push(Suggestion {
                action_id: action_id_for(e),
                completion: path_str,
                about: e.about.clone(),
            });
            if out.len() >= limit {
                break;
            }
        }
    }
    out
}

#[tauri::command]
pub fn discovery_suggest(typed: String, limit: Option<usize>) -> Result<Vec<Suggestion>, String> {
    let catalog = build_catalog();
    let typed = typed
        .strip_prefix("vox ")
        .unwrap_or(&typed)
        .trim()
        .to_string();
    Ok(match_catalog(&catalog.entries, &typed, limit.unwrap_or(8)))
}

/// Rich help for one action id, for the discovery rail.
#[derive(Debug, Clone, Serialize)]
pub struct ActionHelp {
    pub action_id: String,
    pub about: String,
    pub args: Vec<ArgHelp>,
    pub example: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArgHelp {
    pub name: String,
    pub help: String,
    pub required: bool,
}

#[tauri::command]
pub fn discovery_help(action_id: String) -> Result<Option<ActionHelp>, String> {
    let catalog = build_catalog();
    let entry = catalog
        .entries
        .iter()
        .find(|e| action_id_for(e) == action_id);
    Ok(entry.map(|e| {
        let args = e
            .arguments
            .iter()
            .map(|a| ArgHelp {
                name: a.long.clone().unwrap_or_else(|| a.name.clone()),
                help: a.help.clone().unwrap_or_default(),
                required: a.required,
            })
            .collect();
        ActionHelp {
            action_id: action_id.clone(),
            about: e.about.clone(),
            args,
            example: format!("vox {}", e.path.join(" ")),
        }
    }))
}

/// Record an exposure (seen/used) for the current user. `used=true` ⇒ Recall::Used.
#[tauri::command]
pub async fn discovery_record(
    action_id: String,
    used: bool,
    now_ms: i64,
    dwell_ms: i64,
) -> Result<(), String> {
    let config = vox_db::DbConfig::resolve_for_mesh().map_err(|e| e.to_string())?;
    let db = vox_db::Codex::connect(config)
        .await
        .map_err(|e| e.to_string())?;
    let user_id = vox_gamify::db::canonical_user_id();
    let recall = if used {
        vox_gamify::discovery::Recall::Used
    } else {
        vox_gamify::discovery::Recall::Seen
    };
    vox_gamify::discovery::ledger::record(&db, &user_id, &action_id, recall, now_ms, dwell_ms)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_lists_runnable_leaves() {
        let catalog = build_catalog();
        let s = match_catalog(&catalog.entries, "", 5);
        assert!(!s.is_empty());
        assert!(s.len() <= 5);
    }

    #[test]
    fn prefix_filters_to_matching_paths() {
        let catalog = build_catalog();
        // "config" is a stable top-level group across the CLI.
        let s = match_catalog(&catalog.entries, "config", 20);
        assert!(
            s.iter()
                .all(|x| x.completion.starts_with("config") || x.action_id.contains("config")),
            "got: {:?}",
            s.iter().map(|x| &x.completion).collect::<Vec<_>>()
        );
    }
}
