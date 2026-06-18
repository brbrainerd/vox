use std::path::Path;
use std::process::Command;

/// Rule violation details.
#[derive(Debug)]
pub struct CommitViolation {
    pub commit: String,
    pub summary: String,
    pub message: String,
    pub reason: String,
}

/// Check commits from `base` to `HEAD` for:
/// 1. Conventional Commit compliance.
/// 2. Line-churn limits (max 800 lines for chore/docs/style/ci/test unless whitelisted).
pub fn run(workspace_root: &Path, base: &str) -> anyhow::Result<Vec<CommitViolation>> {
    let mut violations = Vec::new();

    // Get list of commit hashes in the range
    let output = Command::new("git")
        .args(["rev-list", &format!("{}..HEAD", base)])
        .current_dir(workspace_root)
        .output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git rev-list failed: {}", err.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let commits: Vec<&str> = stdout
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect();

    for commit in commits {
        // Check parent count (merge check)
        let parent_output = Command::new("git")
            .args(["rev-list", "--parents", "-n", "1", commit])
            .current_dir(workspace_root)
            .output()?;

        let parent_str = String::from_utf8_lossy(&parent_output.stdout);
        let parents: Vec<&str> = parent_str.split_whitespace().collect();
        // If parents.len() > 2, it means there are 2 or more parents, i.e., it's a merge commit.
        let is_merge = parents.len() > 2;

        // Get commit message
        let msg_output = Command::new("git")
            .args(["log", "--format=%B", "-n", "1", commit])
            .current_dir(workspace_root)
            .output()?;

        let message = String::from_utf8_lossy(&msg_output.stdout).to_string();
        let first_line = message.lines().next().unwrap_or("").trim().to_string();

        if is_merge {
            // Merge commits are exempt from conventional commit checks and line limits.
            continue;
        }

        // 1. Enforce Conventional Commit formatting
        let parsed = parse_conventional_commit(&first_line);
        let (commit_type, is_conventional) = match parsed {
            Some(t) => (t, true),
            None => {
                violations.push(CommitViolation {
                    commit: commit.to_string(),
                    summary: first_line.clone(),
                    message: message.clone(),
                    reason: "Commit message does not follow Conventional Commits standard (e.g., 'feat(scope): message' or 'fix: message')".to_string(),
                });
                ("unknown", false)
            }
        };

        if !is_conventional {
            continue;
        }

        // 2. Check line churn limits if type is chore/docs/style/ci/test
        let churn_limiting_types = ["chore", "docs", "style", "ci", "test"];
        if churn_limiting_types.contains(&commit_type) {
            // Get diff stat/numstat
            let diff_output = Command::new("git")
                .args(["show", "--numstat", "--format=", commit])
                .current_dir(workspace_root)
                .output()?;

            let diff_stdout = String::from_utf8_lossy(&diff_output.stdout);
            let mut total_churn = 0;
            for line in diff_stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 3 {
                    continue;
                }
                let filepath = parts[2];
                if is_whitelisted(filepath) {
                    continue;
                }
                let added: u32 = parts[0].parse().unwrap_or(0);
                let deleted: u32 = parts[1].parse().unwrap_or(0);
                total_churn += added + deleted;
            }

            if total_churn > 800 {
                violations.push(CommitViolation {
                    commit: commit.to_string(),
                    summary: first_line.clone(),
                    message: message.clone(),
                    reason: format!(
                        "Commit of type '{}' has {} changed lines (threshold: 800 lines for non-whitelisted files)",
                        commit_type, total_churn
                    ),
                });
            }
        }
    }

    Ok(violations)
}

/// Simple conventional commit parser.
/// Returns Some(type) if valid.
fn parse_conventional_commit(first_line: &str) -> Option<&str> {
    let line = first_line.trim();
    if line.is_empty() {
        return None;
    }

    // Split type/scope and description at the first colon
    let colon_idx = line.find(':')?;
    let header = &line[..colon_idx];
    let description = &line[colon_idx + 1..];

    if description.trim().is_empty() {
        return None;
    }

    // Split type and scope (if present)
    let type_part = if let Some(open_paren) = header.find('(') {
        let close_paren = header.find(')')?;
        if close_paren < open_paren {
            return None;
        }
        &header[..open_paren]
    } else {
        header
    };

    let commit_type = type_part.trim_end_matches('!'); // Allow breaking change exclamation mark

    let allowed_types = [
        "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore",
        "revert",
    ];

    if allowed_types.contains(&commit_type) {
        Some(commit_type)
    } else {
        None
    }
}

/// Check if a file should be ignored from the line-churn calculations.
fn is_whitelisted(path: &str) -> bool {
    let p = path.replace('\\', "/");
    p.ends_with(".generated.ts")
        || p.ends_with(".generated.md")
        || p.ends_with(".generated.json")
        || p.starts_with("examples/golden/")
        || p.starts_with("third_party/")
        || p.starts_with("vendor/")
        || p == "Cargo.lock"
        || p == "Cargo.toml"
        || p == "pnpm-lock.yaml"
        || p == ".gitattributes"
        || p.starts_with("contracts/reports/")
        || p == "crates/vox-cli/tests/fixtures/command_catalog_paths_baseline.txt"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_conventional_commit() {
        assert_eq!(
            parse_conventional_commit("feat: add something"),
            Some("feat")
        );
        assert_eq!(
            parse_conventional_commit("fix(gui): resolve crash"),
            Some("fix")
        );
        assert_eq!(
            parse_conventional_commit("chore!: major refactor"),
            Some("chore")
        );
        assert_eq!(parse_conventional_commit("invalid_type: no"), None);
        assert_eq!(parse_conventional_commit("feat(scope):"), None);
        assert_eq!(parse_conventional_commit("Merge branch 'main'"), None);
    }

    #[test]
    fn test_is_whitelisted() {
        assert!(is_whitelisted("examples/golden/file.vox"));
        assert!(is_whitelisted("Cargo.lock"));
        assert!(is_whitelisted("ui/src/generated/registry.generated.ts"));
        assert!(!is_whitelisted("crates/vox-cli/src/main.rs"));
    }
}
