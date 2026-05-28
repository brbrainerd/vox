use std::collections::HashSet;
use std::path::Path;

use clap::{Command, CommandFactory, ValueEnum};
use serde::Serialize;

use crate::command_contract;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum CatalogFormat {
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogTier {
    Recommended,
    Advanced,
    FeatureGated,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandCatalogArgument {
    pub name: String,
    pub short: Option<char>,
    pub long: Option<String>,
    pub help: Option<String>,
    pub required: bool,
    pub takes_value: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandCatalogEntry {
    pub path: Vec<String>,
    pub command: String,
    pub about: String,
    pub aliases: Vec<String>,
    pub has_subcommands: bool,
    pub compiled_in: bool,
    pub source_group: String,
    pub feature_gate: Option<String>,
    pub tier: CatalogTier,
    /// Transport-independent capability id (`cli.*`) when `contracts/capability` loads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_id: Option<String>,
    #[serde(default)]
    pub arguments: Vec<CommandCatalogArgument>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandCatalog {
    pub generated_from: String,
    pub entries: Vec<CommandCatalogEntry>,
}

pub fn build_catalog() -> CommandCatalog {
    let root = crate::VoxCliRoot::command();
    let mut entries = Vec::new();
    // Iterative DFS: the `vox` CLI tree is deep enough that recursive walks can overflow the
    // default Windows thread stack (~1 MiB) when many tests run in parallel.
    let mut stack: Vec<(&Command, Vec<String>)> = Vec::new();
    let top: Vec<_> = root.get_subcommands().collect();
    for sub in top.into_iter().rev() {
        stack.push((sub, vec![sub.get_name().to_string()]));
    }
    while let Some((cmd, path)) = stack.pop() {
        push_catalog_entry(cmd, &path, &mut entries);
        let children: Vec<_> = cmd.get_subcommands().collect();
        for sub in children.into_iter().rev() {
            let mut child_path = path.clone();
            child_path.push(sub.get_name().to_string());
            stack.push((sub, child_path));
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    apply_capability_ids(&mut entries, &repo_root);
    CommandCatalog {
        generated_from: "clap::CommandFactory(VoxCliRoot)".to_string(),
        entries,
    }
}

fn apply_capability_ids(entries: &mut [CommandCatalogEntry], repo_root: &Path) {
    let Ok(doc) = vox_capability_registry::load_document(repo_root) else {
        return;
    };
    let exempt: HashSet<Vec<String>> = doc
        .exemptions
        .as_ref()
        .map(|e| e.cli_paths.iter().cloned().collect())
        .unwrap_or_default();
    for e in entries.iter_mut() {
        if exempt.contains(&e.path) {
            e.capability_id = None;
            continue;
        }
        e.capability_id = Some(vox_capability_registry::implicit_cli_capability_id(&e.path));
    }
}

pub fn render_text(entries: &[CommandCatalogEntry]) -> String {
    let mut out = String::new();
    out.push_str("Vox command catalog (clap-derived)\n");
    out.push_str("Command | Tier | Description\n");
    out.push_str("------- | ---- | -----------\n");
    for e in entries {
        let tier = match e.tier {
            CatalogTier::Recommended => "recommended",
            CatalogTier::Advanced => "advanced",
            CatalogTier::FeatureGated => "feature_gated",
        };
        out.push_str(&format!(
            "{} | {} | {}\n",
            e.command,
            tier,
            sanitize_about(&e.about)
        ));
    }
    out
}

pub fn select_entries(
    all_entries: Vec<CommandCatalogEntry>,
    recommended_only: bool,
    include_nested: bool,
) -> Vec<CommandCatalogEntry> {
    all_entries
        .into_iter()
        .filter(|e| include_nested || e.path.len() == 1)
        .filter(|e| !recommended_only || e.tier == CatalogTier::Recommended)
        .collect()
}

/// Filter and rank `entries` by fuzzy match against `pattern`.
///
/// Matches against the command path joined as a string (e.g. `"vox shell check"`)
/// and the `about` description.  Returns entries sorted by descending score with
/// zero-score (no match) entries excluded.
///
/// When the `fuzzy-search` feature is disabled, falls back to a case-insensitive
/// substring filter so call-sites work unconditionally.
/// Build the single searchable string for an entry.
/// Includes command path, all aliases, and the about text so that e.g.
/// searching `"fab"` surfaces `vox fabrica` (alias: `fab`).
#[cfg(feature = "fuzzy-search")]
fn entry_search_key(e: &CommandCatalogEntry) -> String {
    if e.aliases.is_empty() {
        format!("{} — {}", e.command, e.about)
    } else {
        format!("{} ({}) — {}", e.command, e.aliases.join(", "), e.about)
    }
}

/// Filter and rank `entries` by fuzzy match against `pattern`.
///
/// Candidates are built from `command`, all `aliases`, and `about` so that
/// alias-only matches (e.g. `"fab"` → `vox fabrica`) are surfaced.
/// Returns entries sorted by descending score; zero-score entries excluded.
///
/// Falls back to case-insensitive substring filter when `fuzzy-search` is disabled.
pub fn search_entries(
    entries: Vec<CommandCatalogEntry>,
    pattern: &str,
) -> Vec<CommandCatalogEntry> {
    search_entries_scored(entries, pattern)
        .into_iter()
        .map(|s| s.entry)
        .collect()
}

/// A search hit carrying both the matched entry and its match score.
///
/// `score` is non-zero when the `fuzzy-search` feature is enabled (nucleo
/// score; higher = better match) and `0` for the substring-fallback path
/// (where ordering is preserved from the input).
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub entry: CommandCatalogEntry,
    pub score: u32,
    /// Which field the pattern matched against ("command", "alias", or "about").
    /// Best-effort: only populated by the substring-fallback path; the fuzzy
    /// path scores against a concatenated key so it cannot attribute precisely.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_via: Option<String>,
}

/// Same as [`search_entries`] but returns scores and match attribution for
/// JSON output and richer rendering.
pub fn search_entries_scored(
    entries: Vec<CommandCatalogEntry>,
    pattern: &str,
) -> Vec<SearchResult> {
    if pattern.is_empty() {
        return entries
            .into_iter()
            .map(|entry| SearchResult {
                entry,
                score: 0,
                matched_via: None,
            })
            .collect();
    }

    #[cfg(feature = "fuzzy-search")]
    {
        let mut matcher = crate::fuzzy::FuzzyMatcher::new();
        let candidates: Vec<String> = entries.iter().map(entry_search_key).collect();
        matcher
            .rank(pattern, &candidates)
            .into_iter()
            .map(|(idx, score)| SearchResult {
                entry: entries[idx].clone(),
                score,
                matched_via: None,
            })
            .collect()
    }

    #[cfg(not(feature = "fuzzy-search"))]
    {
        let pat = pattern.to_ascii_lowercase();
        entries
            .into_iter()
            .filter_map(|entry| {
                let matched_via = if entry.command.to_ascii_lowercase().contains(&pat) {
                    Some("command".to_owned())
                } else if entry
                    .aliases
                    .iter()
                    .any(|a| a.to_ascii_lowercase().contains(&pat))
                {
                    Some("alias".to_owned())
                } else if entry.about.to_ascii_lowercase().contains(&pat) {
                    Some("about".to_owned())
                } else {
                    return None;
                };
                Some(SearchResult {
                    entry,
                    score: 0,
                    matched_via,
                })
            })
            .collect()
    }
}

/// JSON wrapper for search-mode output (richer than [`CommandCatalog`]).
#[derive(Debug, Clone, Serialize)]
pub struct SearchOutput {
    pub generated_from: String,
    pub pattern: String,
    pub match_count: usize,
    pub results: Vec<SearchResult>,
}

/// Render search results with alias context shown inline.
///
/// Differs from [`render_text`] in that the aliases column is included so users
/// can see which alias triggered the match.
pub fn render_search_results(entries: &[CommandCatalogEntry], pattern: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Search results for {:?} ({} match{})\n",
        pattern,
        entries.len(),
        if entries.len() == 1 { "" } else { "es" }
    ));
    out.push_str("Command | Aliases | Tier | Description\n");
    out.push_str("------- | ------- | ---- | -----------\n");
    for e in entries {
        let tier = match e.tier {
            CatalogTier::Recommended => "recommended",
            CatalogTier::Advanced => "advanced",
            CatalogTier::FeatureGated => "feature_gated",
        };
        let aliases = if e.aliases.is_empty() {
            "-".to_owned()
        } else {
            e.aliases.join(", ")
        };
        out.push_str(&format!(
            "{} | {} | {} | {}\n",
            e.command,
            aliases,
            tier,
            sanitize_about(&e.about)
        ));
    }
    out
}

fn push_catalog_entry(cmd: &Command, path: &[String], out: &mut Vec<CommandCatalogEntry>) {
    let feature_gate = command_contract::merged_feature_gate(path);
    let tier = tier_for_path(path, feature_gate.is_some());
    out.push(CommandCatalogEntry {
        command: format!("vox {}", path.join(" ")),
        about: cmd
            .get_about()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "(no description)".to_string()),
        aliases: collect_aliases(cmd),
        has_subcommands: cmd.has_subcommands(),
        compiled_in: true,
        source_group: command_contract::catalog_source_group(path),
        feature_gate,
        path: path.to_vec(),
        tier,
        capability_id: None,
        arguments: cmd
            .get_arguments()
            .map(|arg| CommandCatalogArgument {
                name: arg.get_id().to_string(),
                short: arg.get_short(),
                long: arg.get_long().map(|s| s.to_string()),
                help: arg.get_help().map(|s| s.to_string()),
                required: arg.is_required_set(),
                takes_value: matches!(arg.get_action(), clap::ArgAction::Set | clap::ArgAction::Append),
            })
            .collect(),
    });
}

fn collect_aliases(cmd: &Command) -> Vec<String> {
    let mut aliases: Vec<String> = cmd.get_all_aliases().map(ToString::to_string).collect();
    aliases.sort();
    aliases.dedup();
    aliases
}

fn tier_for_path(path: &[String], feature_gated: bool) -> CatalogTier {
    if feature_gated {
        return CatalogTier::FeatureGated;
    }
    let top = path.first().map(String::as_str).unwrap_or_default();
    if path.len() == 1
        && matches!(
            top,
            "build" | "check" | "run" | "test" | "bundle" | "dev" | "doctor" | "completions"
        )
    {
        return CatalogTier::Recommended;
    }
    CatalogTier::Advanced
}

fn sanitize_about(about: &str) -> String {
    about.replace('\n', " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_catalog_has_expected_top_level_commands() {
        let catalog = build_catalog();
        let commands: Vec<&str> = catalog
            .entries
            .iter()
            .filter(|e| e.path.len() == 1)
            .map(|e| e.path[0].as_str())
            .collect();
        for required in ["build", "check", "run", "doctor", "commands", "ci"] {
            assert!(
                commands.contains(&required),
                "missing top-level `{required}`; got {commands:?}"
            );
        }
    }

    #[test]
    fn select_entries_recommended_only_yields_known_starter_set() {
        let catalog = build_catalog();
        let selected = select_entries(catalog.entries, true, false);
        assert!(!selected.is_empty());
        for e in &selected {
            assert_eq!(
                e.tier,
                CatalogTier::Recommended,
                "{} should be recommended tier",
                e.command
            );
            assert_eq!(e.path.len(), 1, "{} should be top-level only", e.command);
        }
        let names: Vec<&str> = selected.iter().map(|e| e.path[0].as_str()).collect();
        for starter in [
            "build",
            "check",
            "run",
            "test",
            "bundle",
            "dev",
            "doctor",
            "completions",
        ] {
            assert!(
                names.contains(&starter),
                "recommended set should include `{starter}`; got {names:?}"
            );
        }
    }

    #[test]
    fn select_entries_top_level_excludes_nested_paths() {
        let catalog = build_catalog();
        let flat = select_entries(catalog.entries.clone(), false, false);
        assert!(flat.iter().all(|e| e.path.len() == 1));

        let nested = select_entries(catalog.entries, false, true);
        assert!(
            nested.iter().any(|e| e.path.len() > 1),
            "with include_nested, expect at least one multi-segment path"
        );
    }

    #[test]
    fn feature_gated_commands_marked_tier() {
        let catalog = build_catalog();
        let mens = catalog
            .entries
            .iter()
            .find(|e| e.path == ["mens"])
            .expect("default vox-cli build includes `mens`");
        assert_eq!(mens.tier, CatalogTier::FeatureGated);
        assert_eq!(mens.feature_gate.as_deref(), Some("mens-base|gpu"));
    }

    #[test]
    fn render_text_contains_command_and_tier_labels() {
        let catalog = build_catalog();
        let subset: Vec<_> = catalog
            .entries
            .into_iter()
            .filter(|e| e.path == ["build"])
            .collect();
        assert_eq!(subset.len(), 1);
        let text = render_text(&subset);
        assert!(text.contains("vox build"));
        assert!(text.contains("recommended"));
    }

    #[test]
    fn search_entries_returns_matches() {
        let catalog = build_catalog();
        // "shell" should match `vox shell` and `vox shell check`/`vox shell repl`.
        let results = search_entries(catalog.entries, "shell");
        assert!(
            !results.is_empty(),
            "search for 'shell' should return at least one entry"
        );
        assert!(
            results.iter().any(|e| e.command.contains("shell")),
            "expected a 'shell' command in results; got: {:?}",
            results.iter().map(|e| &e.command).collect::<Vec<_>>()
        );
    }

    #[test]
    fn search_entries_empty_pattern_returns_all() {
        let catalog = build_catalog();
        let total = catalog.entries.len();
        let results = search_entries(catalog.entries, "");
        assert_eq!(results.len(), total);
    }

    #[test]
    fn search_entries_no_match_returns_empty() {
        let catalog = build_catalog();
        let results = search_entries(catalog.entries, "zzz_no_such_command_xyzzy");
        assert!(
            results.is_empty(),
            "expected no results for nonsense pattern"
        );
    }
}
