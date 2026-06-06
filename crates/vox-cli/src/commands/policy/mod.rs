//! `vox policy` — read-only view over the unified policy catalog.

pub mod status_writer;

use clap::Subcommand;
use vox_config::{PolicyEntry, PolicyRegistry};

#[derive(Debug, Subcommand)]
pub enum PolicyCmd {
    /// List policies, optionally filtered by domain or group.
    List {
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        group: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show full detail (including rule contents) for one policy id.
    Show { id: String },
    /// List the distinct domains present in the catalog.
    Domains,
    /// List the distinct group labels present in the catalog.
    Groups,
}

fn matches_filter(e: &PolicyEntry, domain: &Option<String>, group: &Option<String>) -> bool {
    let dom_ok = domain
        .as_deref()
        // Match against the serialized kebab-case domain (e.g. `ci-gate`), not the
        // PascalCase `Debug` form — `--domain ci-gate` must select `CiGate` rows.
        .map(|d| serde_domain(e).to_lowercase().contains(&d.to_lowercase()))
        .unwrap_or(true);
    let grp_ok = group
        .as_deref()
        .map(|g| e.group.to_lowercase().contains(&g.to_lowercase()))
        .unwrap_or(true);
    dom_ok && grp_ok
}

fn serde_domain(e: &PolicyEntry) -> String {
    serde_yaml::to_string(&e.domain)
        .unwrap_or_default()
        .trim()
        .trim_matches('"')
        .to_string()
}

fn render_show(e: &PolicyEntry) -> String {
    let sev = e
        .severity
        .map(|s| format!("{s:?}").to_lowercase())
        .unwrap_or_else(|| "-".into());
    format!(
        "{id}\n  {title}\n  domain:   {domain}\n  group:    {group}\n  severity: {sev}{blocking}\n  runs on:  {runs}\n  origin:   {origin}\n\n  {desc}\n\n  --- rule contents ---\n  kind:   {kind}\n  source: {source}\n  detail: {detail}\n",
        id = e.id,
        title = e.title,
        domain = serde_domain(e),
        group = e.group,
        blocking = if e.blocking { " (blocking)" } else { "" },
        runs = e.runs_on.join(", "),
        origin = e.origin,
        desc = e.description,
        kind = format!("{:?}", e.source.kind).to_lowercase(),
        source = e.source.reference,
        detail = e.source.detail.as_deref().unwrap_or("(none)"),
    )
}

/// Entry point for `vox policy <cmd>`.
pub fn run(cmd: PolicyCmd, repo_root: &std::path::Path) -> anyhow::Result<()> {
    let reg: PolicyRegistry =
        vox_config::load_policy_registry(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    match cmd {
        PolicyCmd::List {
            domain,
            group,
            json,
        } => {
            let items: Vec<&PolicyEntry> = reg
                .policies
                .iter()
                .filter(|e| matches_filter(e, &domain, &group))
                .collect();
            if json {
                println!("{}", serde_json::to_string_pretty(&items)?);
            } else {
                for e in items {
                    let sev = e
                        .severity
                        .map(|s| format!("{s:?}").to_lowercase())
                        .unwrap_or_else(|| "-".into());
                    println!(
                        "{:<40} [{}]{}  {}",
                        e.id,
                        sev,
                        if e.blocking { " blocking" } else { "" },
                        e.title
                    );
                }
            }
        }
        PolicyCmd::Show { id } => match reg.policies.iter().find(|e| e.id == id) {
            Some(e) => print!("{}", render_show(e)),
            None => anyhow::bail!("no policy with id `{id}` (try `vox policy list`)"),
        },
        PolicyCmd::Domains => {
            let mut ds: Vec<String> = reg.policies.iter().map(serde_domain).collect();
            ds.sort();
            ds.dedup();
            ds.iter().for_each(|d| println!("{d}"));
        }
        PolicyCmd::Groups => {
            let mut gs: Vec<String> = reg.policies.iter().map(|e| e.group.clone()).collect();
            gs.sort();
            gs.dedup();
            gs.iter().for_each(|g| println!("{g}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_config::{PolicyDomain, PolicySource, PolicySourceKind};

    fn entry(id: &str, group: &str) -> PolicyEntry {
        PolicyEntry {
            id: id.into(),
            domain: PolicyDomain::CodeAuditRule,
            title: "T".into(),
            group: group.into(),
            description: "D".into(),
            severity: None,
            blocking: false,
            runs_on: vec![],
            source: PolicySource {
                kind: PolicySourceKind::Pattern,
                reference: "r".into(),
                detail: None,
            },
            docs: None,
            default_enabled: true,
            protected: false,
            origin: "builtin".into(),
        }
    }

    #[test]
    fn group_filter_is_case_insensitive_substring() {
        let e = entry("code-audit/stub/todo", "Language rules / Stubs (TOESTUB)");
        assert!(matches_filter(&e, &None, &Some("stubs".into())));
        assert!(!matches_filter(&e, &None, &Some("architecture".into())));
    }

    #[test]
    fn domain_filter_matches_kebab_case_serialized_domain() {
        let mut e = entry("ci-gate/ci.foo", "CI Gates / ci");
        e.domain = PolicyDomain::CiGate;
        assert!(matches_filter(&e, &Some("ci-gate".into()), &None));
        assert!(!matches_filter(&e, &Some("arch-rule".into()), &None));
    }

    #[test]
    fn show_includes_rule_contents() {
        let out = render_show(&entry("code-audit/stub/todo", "G"));
        assert!(out.contains("rule contents"));
        assert!(out.contains("source: r"));
    }
}
