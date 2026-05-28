//! `vox ci artifact-audit` / `artifact-prune` — policy-driven workspace artifact inventory and cleanup.

mod retention;

use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use chrono::{TimeZone, Utc};
use serde::Serialize;
use walkdir::WalkDir;

pub use retention::WorkspaceArtifactRetentionFile;

use crate::artifact_policy;
use retention::{
    age_days, collect_stale_rename_paths, is_scratch_root_file, path_allowed_for_prune,
    plan_mens_run_deletions, repo_root_stale_target_dirs,
};

const DEFAULT_POLICY_REL: &str = "contracts/operations/workspace-artifact-retention.v1.yaml";

#[derive(Serialize)]
pub struct ArtifactAuditRow {
    pub path: String,
    pub class: String,
    pub bytes: u64,
    pub age_days: u32,
    pub last_modified: Option<String>,
    pub tracked: bool,
    pub untracked: bool,
    pub delete_candidate: bool,
    pub delete_reason: Option<String>,
}

fn default_policy_path(root: &Path) -> PathBuf {
    root.join(DEFAULT_POLICY_REL)
}

fn system_time_rfc3339(t: SystemTime) -> Option<String> {
    let secs = t.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let secs_i = i64::try_from(secs).ok()?;
    Utc.timestamp_opt(secs_i, 0)
        .single()
        .map(|d| d.to_rfc3339())
}

fn path_size_bytes(path: &Path) -> u64 {
    let Ok(m) = fs::symlink_metadata(path) else {
        return 0;
    };
    if m.is_file() {
        return m.len();
    }
    if !m.is_dir() {
        return 0;
    }
    let mut n = 0u64;
    for e in WalkDir::new(path).into_iter().filter_map(Result::ok) {
        if e.path_is_symlink() {
            continue;
        }
        if let Ok(mm) = e.metadata() {
            if mm.is_file() {
                n = n.saturating_add(mm.len());
            }
        }
    }
    n
}

fn git_lists_path(repo: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(repo) else {
        return false;
    };
    let mut rel_s = rel.to_string_lossy().replace('\\', "/");
    if path.is_dir() && !rel_s.is_empty() && !rel_s.ends_with('/') {
        rel_s.push('/');
    }
    let output = match // vox-arch-check: allow git-exec
        Command::new("git")
        .current_dir(repo)
        .args(["ls-files", "-z", "--", &rel_s])
        .output()
    {
        Ok(o) => o,
        Err(_) => return false,
    };
    !output.stdout.is_empty()
}

fn is_symlink_path(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.is_symlink())
        .unwrap_or(false)
}

/// When `delete_candidate_override` is `Some`, it wins (after [`refresh_tracked`] may still clear it).
fn classify_row(
    path: PathBuf,
    class: &str,
    delete_reason: Option<String>,
    delete_candidate_override: Option<bool>,
) -> ArtifactAuditRow {
    let mtime = fs::symlink_metadata(&path)
        .and_then(|m| m.modified())
        .unwrap_or(UNIX_EPOCH);
    let bytes = path_size_bytes(&path);
    let tracked = false;
    let delete_candidate = delete_candidate_override.unwrap_or_else(|| delete_reason.is_some());
    ArtifactAuditRow {
        path: path.to_string_lossy().to_string(),
        class: class.to_string(),
        bytes,
        age_days: age_days(mtime),
        last_modified: system_time_rfc3339(mtime),
        tracked,
        untracked: !tracked,
        delete_candidate,
        delete_reason,
    }
}

fn refresh_tracked(repo: &Path, row: &mut ArtifactAuditRow, path: &Path) {
    row.tracked = git_lists_path(repo, path);
    row.untracked = !row.tracked;
    if row.tracked {
        row.delete_candidate = false;
        row.delete_reason = Some("git-tracked (skip)".into());
    }
}

/// Git-tracked trees under `mens/runs` cannot be pruned; huge ones are almost always a mistake.
const ADVISORY_LARGE_TRACKED_MENS_BYTES: u64 = 256 * 1024 * 1024;

fn emit_large_tracked_mens_advisories(rows: &[ArtifactAuditRow]) {
    for r in rows {
        if r.class == "MensRun" && r.tracked && r.bytes >= ADVISORY_LARGE_TRACKED_MENS_BYTES {
            let gb = r.bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            eprintln!(
                "[advisory] git-tracked mens run {:.2} GiB — prune will never delete tracked files; \
                 if these are generated artifacts, stop tracking them (backup, then `git rm -r --cached <path>`): {}",
                gb, r.path
            );
        }
    }
}

fn collect_inventory(
    root: &Path,
    policy: &WorkspaceArtifactRetentionFile,
) -> Result<Vec<ArtifactAuditRow>> {
    let (mens_delete, _) = plan_mens_run_deletions(root, &policy.mens)?;
    let mens_set: HashSet<PathBuf> = mens_delete.into_iter().collect();

    let mut rows = Vec::new();

    for p in collect_stale_rename_paths(root) {
        let mut r = classify_row(
            p.clone(),
            "StaleRename",
            Some("stale-rename-suffix".into()),
            None,
        );
        refresh_tracked(root, &mut r, &p);
        rows.push(r);
    }

    for p in repo_root_stale_target_dirs(root) {
        let mut r = classify_row(
            p.clone(),
            "WorkspaceTarget",
            Some("repo-root-target-sprawl".into()),
            None,
        );
        refresh_tracked(root, &mut r, &p);
        rows.push(r);
    }

    for lane in artifact_policy::transient_lane_roots(root) {
        if !lane.is_dir() {
            continue;
        }
        let mtime = fs::metadata(&lane)
            .and_then(|m| m.modified())
            .unwrap_or(UNIX_EPOCH);
        let reason = if age_days(mtime) >= policy.transient.max_age_days {
            Some(format!(
                "transient-target-age>={}d",
                policy.transient.max_age_days
            ))
        } else {
            None
        };
        let mut r = classify_row(lane.clone(), "TransientTarget", reason, None);
        refresh_tracked(root, &mut r, &lane);
        rows.push(r);
    }

    let runs_root = root.join("mens").join("runs");
    if runs_root.is_dir() {
        for entry in
            fs::read_dir(&runs_root).with_context(|| format!("read {}", runs_root.display()))?
        {
            let entry = entry?;
            let ft = entry.file_type()?;
            if ft.is_symlink() || !ft.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "latest" {
                continue;
            }
            let p = entry.path();
            let reason = if mens_set.contains(&p) {
                Some("mens-retention-policy".into())
            } else {
                None
            };
            let mut r = classify_row(p.clone(), "MensRun", reason, None);
            refresh_tracked(root, &mut r, &p);
            rows.push(r);
        }
    }

    let Ok(rd) = fs::read_dir(root) else {
        return Ok(rows);
    };
    for e in rd.filter_map(Result::ok) {
        let name = e.file_name().to_string_lossy().to_string();
        if !is_scratch_root_file(&name) {
            continue;
        }
        let p = e.path();
        let Ok(ft) = e.file_type() else {
            continue;
        };
        if !ft.is_file() {
            continue;
        }
        let mtime = fs::metadata(&p)
            .and_then(|m| m.modified())
            .unwrap_or(UNIX_EPOCH);
        let reason = if age_days(mtime) >= policy.scratch.max_age_days {
            Some(format!(
                "scratch-file-age>={}d",
                policy.scratch.max_age_days
            ))
        } else {
            None
        };
        let mut r = classify_row(p.clone(), "ScratchLog", reason, None);
        refresh_tracked(root, &mut r, &p);
        rows.push(r);
    }

    let cw = artifact_policy::canonical_workspace_target(root);
    if cw.is_dir() {
        let mut r = classify_row(
            cw.clone(),
            "WorkspaceTarget",
            Some("canonical Cargo target — not removed by artifact-prune; use cargo clean".into()),
            Some(false),
        );
        refresh_tracked(root, &mut r, &cw);
        rows.push(r);
    }

    Ok(rows)
}

pub fn run_audit(root: &Path, json: bool) -> Result<()> {
    let policy_path = default_policy_path(root);
    let policy = if policy_path.is_file() {
        WorkspaceArtifactRetentionFile::load(&policy_path)?
    } else {
        WorkspaceArtifactRetentionFile::embedded_defaults()
    };

    let rows = collect_inventory(root, &policy)?;
    emit_large_tracked_mens_advisories(&rows);

    if json {
        let s = serde_json::to_string_pretty(&rows)?;
        println!("{s}");
    } else {
        println!(
            "artifact-audit: policy={} rows={}",
            policy_path.display(),
            rows.len()
        );
        for r in &rows {
            let dc = if r.delete_candidate { "Y" } else { "n" };
            let tr = if r.tracked { "tracked" } else { "untracked" };
            println!(
                "{}\t{}\t{}B\t{}d\t{}\t{}\t{:?}",
                r.path, r.class, r.bytes, r.age_days, tr, dc, r.delete_reason
            );
        }
    }
    Ok(())
}

fn unix_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn delete_path_logged(path: &Path, dry_run: bool, class: &str, reason: &str) -> Result<u64> {
    let bytes = path_size_bytes(path);
    if dry_run {
        println!(
            "[dry-run] class={class} reason={reason} bytes={bytes} path={}",
            path.display()
        );
        return Ok(bytes);
    }
    eprintln!(
        "[delete] class={class} reason={reason} bytes={bytes} path={}",
        path.display()
    );
    if is_symlink_path(path) {
        eprintln!("[skip] symlink path={}", path.display());
        return Ok(0);
    }
    let meta = fs::symlink_metadata(path)?;
    let res = if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    if let Err(e) = res {
        let ts = unix_ts();
        let stale_name = format!(
            "{}.stale-{}",
            path.file_name().unwrap_or_default().to_string_lossy(),
            ts
        );
        let alt = path.with_file_name(stale_name);
        fs::rename(path, &alt).with_context(|| {
            format!(
                "delete failed ({e}); rename-to-stale failed path={} alt={}",
                path.display(),
                alt.display()
            )
        })?;
        eprintln!(
            "[stale-rename] path={} -> {}",
            path.display(),
            alt.display()
        );
        return Ok(0);
    }
    Ok(bytes)
}

pub fn run_prune(root: &Path, dry_run: bool, apply: bool, policy_arg: Option<&Path>) -> Result<()> {
    if dry_run && apply {
        return Err(anyhow!("Specify only one of --dry-run or --apply"));
    }
    if !dry_run && !apply {
        return Err(anyhow!("Must specify --dry-run or --apply"));
    }

    let policy_path = policy_arg
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_policy_path(root));
    let policy = if policy_path.is_file() {
        WorkspaceArtifactRetentionFile::load(&policy_path)?
    } else if policy_arg.is_some() {
        return Err(anyhow!(
            "retention policy file not found: {}",
            policy_path.display()
        ));
    } else {
        WorkspaceArtifactRetentionFile::embedded_defaults()
    };

    for w in plan_mens_run_deletions(root, &policy.mens)?.1 {
        eprintln!("[warn] {w}");
    }

    let mut rows = collect_inventory(root, &policy)?;
    for r in &mut rows {
        let p = PathBuf::from(&r.path);
        refresh_tracked(root, r, &p);
    }
    emit_large_tracked_mens_advisories(&rows);

    let mut queue: VecDeque<(PathBuf, String, String)> = VecDeque::new();

    for r in &rows {
        if !r.delete_candidate {
            continue;
        }
        let p = PathBuf::from(&r.path);
        if !path_allowed_for_prune(&p, root) {
            eprintln!("[skip] not allowed by policy path={}", p.display());
            continue;
        }
        let reason = r.delete_reason.clone().unwrap_or_default();
        queue.push_back((p, r.class.clone(), reason));
    }

    let mut reclaimed = 0u64;
    let mut counts: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();

    while let Some((path, class, reason)) = queue.pop_front() {
        if !path.exists() {
            continue;
        }
        if is_symlink_path(&path) {
            eprintln!("[skip] symlink path={}", path.display());
            continue;
        }
        if git_lists_path(root, &path) {
            eprintln!(
                "[skip] git-tracked path={} (ls-files matched)",
                path.display()
            );
            continue;
        }
        let b = delete_path_logged(&path, dry_run, &class, &reason)?;
        reclaimed = reclaimed.saturating_add(b);
        *counts.entry(class).or_insert(0) += 1;
    }

    println!("artifact-prune: dry_run={dry_run} reclaimed_bytes={reclaimed} per_class={counts:?}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn mens_planner_keeps_min_keep() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let runs = root.join("mens").join("runs");
        fs::create_dir_all(&runs).unwrap();
        for name in ["a", "b", "c"] {
            let p = runs.join(name);
            fs::create_dir(&p).unwrap();
        }
        let policy = retention::MensPolicy {
            max_age_days: 0,
            max_total_bytes: u64::MAX,
            min_keep: 2,
            protected_names: vec![],
            latest_pointer: "mens/runs/latest".into(),
        };
        let (del, _) = plan_mens_run_deletions(root, &policy).unwrap();
        assert!(
            del.len() <= 1,
            "expected at most one deleted when min_keep=2"
        );
    }
}
