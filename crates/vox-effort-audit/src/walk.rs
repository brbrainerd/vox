//! gix-backed commit walker.
//!
//! Commit-graph traversal (sha, parent, message, timestamp, author-email-hash) uses `gix` for
//! pure-Rust speed. The unified-diff body and per-file additions/deletions are sourced from
//! `git diff` (shelled out): `gix 0.70`'s text-diff pretty printer is not yet stable for
//! cross-platform consumption and producing a portable unified diff via the low-level
//! `gix-diff` API would replicate a significant chunk of git's diff formatter. Other
//! consumers in this workspace follow the same gix-for-graph + `git` shell-out pattern
//! (see `crates/vox-git/src/bridge.rs`).

use crate::range::CommitRange;
use chrono::{DateTime, TimeZone, Utc};
use sha2::{Digest, Sha256};
use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct CommitRecord {
    pub sha: String,
    pub parent_sha: Option<String>,
    pub commit_ts: DateTime<Utc>,
    pub message: String,
    pub author_email_sha256: String,
    pub files: Vec<FileChange>,
    pub additions: u64,
    pub deletions: u64,
    pub unified_diff_text: String,
    pub diff_truncated: bool,
}

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub additions: u64,
    pub deletions: u64,
}

#[derive(Debug, Error)]
pub enum WalkError {
    #[error("git open failed: {0}")]
    Open(String),
    #[error("git walk failed: {0}")]
    Walk(String),
}

/// Iterates commits in the range, newest-first.
pub fn iter_commits(
    repo_path: &Path,
    range: &CommitRange,
    max_diff_bytes: usize,
) -> Result<Vec<CommitRecord>, WalkError> {
    let repo = gix::open(repo_path).map_err(|e| WalkError::Open(e.to_string()))?;

    let (until_id, since_id_opt, time_cutoff): (
        gix::hash::ObjectId,
        Option<gix::hash::ObjectId>,
        Option<DateTime<Utc>>,
    ) = match range {
        CommitRange::Refs { since, until } => {
            let until_id = repo
                .rev_parse_single(until.as_str())
                .map_err(|e| WalkError::Walk(e.to_string()))?
                .detach();
            let since_id = repo
                .rev_parse_single(since.as_str())
                .map_err(|e| WalkError::Walk(e.to_string()))?
                .detach();
            (until_id, Some(since_id), None)
        }
        CommitRange::SinceDuration { duration, until } => {
            let until_id = repo
                .rev_parse_single(until.as_str())
                .map_err(|e| WalkError::Walk(e.to_string()))?
                .detach();
            let cutoff = Utc::now() - *duration;
            (until_id, None, Some(cutoff))
        }
    };

    // BFS from until_id, stopping at since_id (inclusive) or time cutoff.
    let mut records: Vec<CommitRecord> = Vec::new();
    let mut visited: HashSet<gix::hash::ObjectId> = HashSet::new();
    let mut queue: VecDeque<gix::hash::ObjectId> = VecDeque::new();
    queue.push_back(until_id);

    while let Some(oid) = queue.pop_front() {
        if !visited.insert(oid) {
            continue;
        }

        let Some(rec) = build_record(&repo, oid, repo_path, max_diff_bytes)? else {
            continue;
        };

        if let Some(cutoff) = time_cutoff
            && rec.commit_ts < cutoff
        {
            // Past the cutoff window — skip and don't enqueue parents.
            continue;
        }

        records.push(rec);

        // Stop at the `since` ref (inclusive) per spec.
        if let Some(since_id) = since_id_opt
            && oid == since_id
        {
            continue;
        }

        if let Ok(commit) = repo.find_commit(oid) {
            for p in commit.parent_ids() {
                queue.push_back(p.detach());
            }
        }
    }

    // Newest-first.
    records.sort_by(|a, b| b.commit_ts.cmp(&a.commit_ts));
    Ok(records)
}

fn build_record(
    repo: &gix::Repository,
    oid: gix::hash::ObjectId,
    repo_path: &Path,
    max_diff_bytes: usize,
) -> Result<Option<CommitRecord>, WalkError> {
    let commit = match repo.find_commit(oid) {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    let decoded = match commit.decode() {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };

    let sha = oid.to_string();
    let parent_sha = commit.parent_ids().next().map(|p| p.detach().to_string());

    // commit_ts from the commit's time field (seconds since epoch).
    let secs = decoded.time().seconds;
    let commit_ts = Utc.timestamp_opt(secs, 0).single().unwrap_or_else(Utc::now);

    // author email -> sha256 hex
    let email_bytes: &[u8] = decoded.author.email.as_ref();
    let mut hasher = Sha256::new();
    hasher.update(email_bytes);
    let author_email_sha256 = format!("{:x}", hasher.finalize());

    // Full message (subject + body).
    let message_bytes: &[u8] = decoded.message.as_ref();
    let message = String::from_utf8_lossy(message_bytes).into_owned();

    // Per-file additions/deletions from `git diff --numstat`.
    let (files, additions, deletions) = numstat(repo_path, &sha, parent_sha.as_deref());

    // Unified diff body, possibly truncated. Failure here is fatal — a missing
    // diff body would silently degrade the LLM judge's input, so we surface it.
    let (unified_diff_text, diff_truncated) = unified_diff(
        repo_path,
        &sha,
        parent_sha.as_deref(),
        max_diff_bytes,
        &files,
    )?;

    Ok(Some(CommitRecord {
        sha,
        parent_sha,
        commit_ts,
        message,
        author_email_sha256,
        files,
        additions,
        deletions,
        unified_diff_text,
        diff_truncated,
    }))
}

fn numstat(repo_path: &Path, sha: &str, parent_sha: Option<&str>) -> (Vec<FileChange>, u64, u64) {
    let args: Vec<String> = match parent_sha {
        Some(p) => vec![
            "diff".into(),
            "--numstat".into(),
            "-z".into(),
            format!("{p}..{sha}"),
        ],
        None => vec![
            "show".into(),
            "--format=".into(),
            "--numstat".into(),
            "-z".into(),
            sha.into(),
        ],
    };
    let out = match Command::new("git")
        .current_dir(repo_path)
        .args(&args)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(
                sha = %sha,
                error = %e,
                "git numstat spawn failed; treating commit as empty"
            );
            return (Vec::new(), 0, 0);
        }
    };
    if !out.status.success() {
        // Per-file stats are best-effort: a binary-only commit or a transient
        // git failure should not abort the whole walk. Surface the SHA + stderr
        // in the log so a downstream caller can correlate suspicious empties.
        let stderr = String::from_utf8_lossy(&out.stderr);
        tracing::warn!(
            sha = %sha,
            code = out.status.code().unwrap_or(-1),
            stderr = %stderr.trim(),
            "git numstat exited non-zero; treating commit as empty"
        );
        return (Vec::new(), 0, 0);
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut files = Vec::new();
    let mut total_add = 0u64;
    let mut total_del = 0u64;
    // numstat -z: "ADDS\tDELS\tPATH\0" (binary marks ADDS/DELS as "-").
    for entry in stdout.split('\0').filter(|s| !s.is_empty()) {
        let mut parts = entry.splitn(3, '\t');
        let adds_s = parts.next().unwrap_or("0");
        let dels_s = parts.next().unwrap_or("0");
        let path = parts.next().unwrap_or("").to_string();
        if path.is_empty() {
            continue;
        }
        let adds = adds_s.parse::<u64>().unwrap_or(0);
        let dels = dels_s.parse::<u64>().unwrap_or(0);
        total_add += adds;
        total_del += dels;
        files.push(FileChange {
            path,
            additions: adds,
            deletions: dels,
        });
    }
    (files, total_add, total_del)
}

fn unified_diff(
    repo_path: &Path,
    sha: &str,
    parent_sha: Option<&str>,
    max_diff_bytes: usize,
    files: &[FileChange],
) -> Result<(String, bool), WalkError> {
    let args: Vec<String> = match parent_sha {
        Some(p) => vec![
            "diff".into(),
            "--unified=3".into(),
            "--no-color".into(),
            format!("{p}..{sha}"),
        ],
        None => vec![
            "show".into(),
            "--format=".into(),
            "--unified=3".into(),
            "--no-color".into(),
            sha.into(),
        ],
    };
    let out = Command::new("git")
        .current_dir(repo_path)
        .args(&args)
        .output()
        .map_err(|e| WalkError::Walk(format!("git diff spawn failed for {sha}: {e}")))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(WalkError::Walk(format!(
            "git diff exited {} for {sha}: {}",
            out.status.code().unwrap_or(-1),
            stderr.trim(),
        )));
    }

    // Size check on the captured stdout vs the configured budget. We avoid
    // re-allocating into a `String` when the diff will be discarded.
    let bytes = &out.stdout;
    if bytes.len() <= max_diff_bytes {
        let text = String::from_utf8_lossy(bytes).into_owned();
        Ok((text, false))
    } else {
        let summary = files
            .iter()
            .map(|f| format!("- {} (+{}/-{})", f.path, f.additions, f.deletions))
            .collect::<Vec<_>>()
            .join("\n");
        Ok((summary, true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::range::CommitRange;

    #[test]
    fn iter_walks_smoke_repo() {
        let (_g, path) = tests_support::make_smoke_repo();
        let range = CommitRange::Refs {
            since: "HEAD~4".into(),
            until: "HEAD".into(),
        };
        let v = iter_commits(&path, &range, 64 * 1024).unwrap();
        assert_eq!(v.len(), 5);
        assert!(v.iter().all(|c| !c.author_email_sha256.is_empty()));
        // Newest-first
        assert!(v[0].commit_ts >= v[4].commit_ts);
    }

    #[test]
    fn unified_diff_surfaces_subprocess_failure() {
        // A real git repo, but ask `git diff` for a bogus SHA pair. `git` will
        // exit non-zero; we expect `WalkError::Walk` rather than a silent empty.
        let (_g, path) = tests_support::make_smoke_repo();
        let result = unified_diff(
            &path,
            "0000000000000000000000000000000000000000",
            Some("1111111111111111111111111111111111111111"),
            64 * 1024,
            &[],
        );
        match result {
            Err(WalkError::Walk(msg)) => {
                assert!(
                    msg.contains("git diff"),
                    "expected error to mention git diff, got: {msg}"
                );
            }
            other => panic!("expected WalkError::Walk, got {other:?}"),
        }
    }

    #[test]
    fn diff_truncation_kicks_in() {
        let (_g, path) = tests_support::make_smoke_repo();
        let range = CommitRange::Refs {
            since: "HEAD~4".into(),
            until: "HEAD".into(),
        };
        let v = iter_commits(&path, &range, 1).unwrap();
        assert!(v.iter().any(|c| c.diff_truncated));
    }
}

// Bridge to tests/support/mod.rs
#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod tests_support;
