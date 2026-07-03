// crates/vox-gui/src/commands/harness_town.rs
//! Thin harness telemetry taps for the Vox Urbs town map (CASTRVM + PORTA).
//! Read-only, slow-poll (the UI polls at 15-30s), fail-honest: any error is
//! returned as Err and rendered as an "unlit" landmark, never fabricated.

use crate::commands::process_util::quiet_command;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CiRunnerDto {
    pub name: String,
    pub busy: bool,
    pub online: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CiFleetDto {
    pub runners: Vec<CiRunnerDto>,
    pub queued: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VcsBranchDto {
    pub name: String,
    pub is_head: bool,
    /// Git's own upstream summary, e.g. "[ahead 2, behind 1]" — verbatim from
    /// `%(upstream:track)`, empty when no upstream. Never synthesized.
    pub track: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VcsPrDto {
    pub number: u64,
    pub title: String,
    pub head_ref: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VcsTownDto {
    pub branches: Vec<VcsBranchDto>,
    /// Empty (with `prs_available=false`) when `gh` is missing/unauthenticated.
    pub prs: Vec<VcsPrDto>,
    pub prs_available: bool,
}

/// Parse `gh api repos/<slug>/actions/runners` JSON (the same source
/// vox-cli's runner_scale.rs reads).
pub(crate) fn parse_runners(json: &str) -> Result<Vec<CiRunnerDto>, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let runners = v["runners"].as_array().ok_or("no .runners array")?;
    Ok(runners
        .iter()
        .map(|r| CiRunnerDto {
            name: r["name"].as_str().unwrap_or("?").to_string(),
            busy: r["busy"].as_bool().unwrap_or(false),
            online: r["status"].as_str() == Some("online"),
        })
        .collect())
}

/// Parse `git for-each-ref refs/heads
/// --format=%(refname:short)%09%(HEAD)%09%(upstream:track)` output.
pub(crate) fn parse_branches(out: &str) -> Vec<VcsBranchDto> {
    out.lines()
        .filter_map(|l| {
            let mut parts = l.split('\t');
            let name = parts.next()?.trim();
            if name.is_empty() {
                return None;
            }
            let is_head = parts.next().map(|h| h.trim() == "*").unwrap_or(false);
            let track = parts.next().unwrap_or("").trim().to_string();
            Some(VcsBranchDto { name: name.to_string(), is_head, track })
        })
        .collect()
}

/// Parse `gh pr list --json number,title,headRefName` output.
pub(crate) fn parse_prs(json: &str) -> Result<Vec<VcsPrDto>, String> {
    let v: Vec<serde_json::Value> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(v.iter()
        .map(|p| VcsPrDto {
            number: p["number"].as_u64().unwrap_or(0),
            title: p["title"].as_str().unwrap_or("").to_string(),
            head_ref: p["headRefName"].as_str().unwrap_or("").to_string(),
        })
        .collect())
}

/// Derive `owner/repo` from a git remote URL (https or ssh).
pub(crate) fn slug_from_remote(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches(".git");
    if let Some(rest) = trimmed.strip_prefix("git@") {
        return rest.split_once(':').map(|(_, s)| s.to_string());
    }
    let no_scheme = trimmed.split("://").nth(1)?;
    let mut seg = no_scheme.splitn(2, '/');
    let _host = seg.next()?;
    Some(seg.next()?.to_string())
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let out = quiet_command(program)
        .args(args)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{program} exited {:?}", out.status.code()));
    }
    String::from_utf8(out.stdout).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn harness_ci_fleet_status() -> Result<CiFleetDto, String> {
    tokio::task::spawn_blocking(|| {
        let remote = run("git", &["remote", "get-url", "origin"])?;
        let slug = slug_from_remote(&remote).ok_or("cannot parse origin remote")?;
        let runners_json = run("gh", &["api", &format!("repos/{slug}/actions/runners")])?;
        let runners = parse_runners(&runners_json)?;
        let queued_json = run(
            "gh",
            &["api", &format!("repos/{slug}/actions/runs?status=queued&per_page=50")],
        )?;
        let queued = serde_json::from_str::<serde_json::Value>(&queued_json)
            .ok()
            .and_then(|v| v["workflow_runs"].as_array().map(|a| a.len() as u32))
            .unwrap_or(0);
        Ok(CiFleetDto { runners, queued })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn vcs_town_status() -> Result<VcsTownDto, String> {
    tokio::task::spawn_blocking(|| {
        let branch_out = run(
            "git",
            &[
                "for-each-ref",
                "refs/heads",
                "--format=%(refname:short)%09%(HEAD)%09%(upstream:track)",
            ],
        )?;
        let branches = parse_branches(&branch_out);
        // PRs are optional: gh missing/unauthenticated → prs_available=false.
        let (prs, prs_available) =
            match run("gh", &["pr", "list", "--json", "number,title,headRefName"]) {
                Ok(json) => (parse_prs(&json).unwrap_or_default(), true),
                Err(_) => (Vec::new(), false),
            };
        Ok(VcsTownDto { branches, prs, prs_available })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_runner_fixture() {
        let json = r#"{"total_count":2,"runners":[
            {"name":"vox-runner-auto-1","status":"online","busy":true},
            {"name":"vox-runner-auto-2","status":"offline","busy":false}
        ]}"#;
        let rows = parse_runners(json).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows[0].busy && rows[0].online);
        assert!(!rows[1].online);
    }

    #[test]
    fn parses_branch_fixture() {
        let out = "main\t \t\nclaude/frosty-fermi\t*\t[ahead 2, behind 1]\n";
        let rows = parse_branches(out);
        assert_eq!(rows.len(), 2);
        assert!(!rows[0].is_head);
        assert_eq!(rows[0].track, "");
        assert!(rows[1].is_head);
        assert_eq!(rows[1].name, "claude/frosty-fermi");
        assert_eq!(rows[1].track, "[ahead 2, behind 1]");
    }

    #[test]
    fn parses_pr_fixture() {
        let json = r#"[{"number":428,"title":"fix guard","headRefName":"fix/guard"}]"#;
        let prs = parse_prs(json).unwrap();
        assert_eq!(prs[0].number, 428);
        assert_eq!(prs[0].head_ref, "fix/guard");
    }

    #[test]
    fn slug_from_https_and_ssh_remotes() {
        assert_eq!(
            slug_from_remote("https://github.com/vox-foundation/vox.git").as_deref(),
            Some("vox-foundation/vox")
        );
        assert_eq!(
            slug_from_remote("git@github.com:vox-foundation/vox.git").as_deref(),
            Some("vox-foundation/vox")
        );
        assert_eq!(slug_from_remote("not a url"), None);
    }

    #[test]
    fn bad_runner_json_is_err_not_empty() {
        assert!(parse_runners("{}").is_err());
        assert!(parse_runners("garbage").is_err());
    }
}
