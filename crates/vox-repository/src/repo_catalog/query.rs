use regex::RegexBuilder;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::{
    CrossRepoQueryTrace, QueryFileParams, QueryHistoryParams, QueryTextParams, RepoAccessMode,
    RepoCatalogError, RepoFileRead, RepoFileReadResponse, RepoHistoryEntry, RepoHistoryResponse,
    RepoQuerySkippedRepository, RepoTextMatch, RepoTextSearchResponse, ResolvedRepoCatalog,
    ResolvedRepositoryDescriptor, SKIP_DIRS,
};

fn unix_ms_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn build_trace(
    workspace_repository_id: &str,
    target_repository_ids: Vec<String>,
    query_kind: &str,
    query_backend: &str,
    source_plane: &str,
    conversation_id: Option<String>,
) -> CrossRepoQueryTrace {
    let started_at_ms = unix_ms_now();
    let suffix = format!("{}-{}-{}", std::process::id(), started_at_ms, query_kind);
    CrossRepoQueryTrace {
        trace_id: format!("xrepo:{suffix}"),
        correlation_id: format!("corr:{suffix}"),
        conversation_id,
        workspace_repository_id: workspace_repository_id.to_string(),
        target_repository_ids,
        source_plane: source_plane.to_string(),
        query_backend: query_backend.to_string(),
        query_kind: query_kind.to_string(),
        started_at_ms,
        completed_at_ms: started_at_ms,
        latency_ms: 0,
    }
}

fn finalize_trace(mut trace: CrossRepoQueryTrace) -> CrossRepoQueryTrace {
    trace.completed_at_ms = unix_ms_now();
    trace.latency_ms = (trace.completed_at_ms - trace.started_at_ms).max(0);
    trace
}

fn selected_repositories<'a>(
    catalog: &'a ResolvedRepoCatalog,
    repository_ids: Option<&[String]>,
) -> Vec<&'a ResolvedRepositoryDescriptor> {
    match repository_ids {
        Some(ids) if !ids.is_empty() => {
            let wanted: HashSet<&str> = ids.iter().map(String::as_str).collect();
            catalog
                .repositories
                .iter()
                .filter(|repo| {
                    repo.repository_id
                        .as_deref()
                        .map(|id| wanted.contains(id))
                        .unwrap_or(false)
                })
                .collect()
        }
        _ => catalog.repositories.iter().collect(),
    }
}

fn candidate_file_is_text(path: &Path, max_file_bytes: usize) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    meta.is_file() && usize::try_from(meta.len()).ok().unwrap_or(usize::MAX) <= max_file_bytes
}

fn repo_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn safe_local_path(root: &Path, rel_or_abs: &str) -> Result<PathBuf, String> {
    crate::resolve_local_path_under_repo_root(root, rel_or_abs)
}

fn skip_non_local_repo(
    repo: &ResolvedRepositoryDescriptor,
    reason: &str,
) -> RepoQuerySkippedRepository {
    RepoQuerySkippedRepository {
        display_name: repo.display_name.clone(),
        repository_id: repo.repository_id.clone(),
        access_mode: repo.access_mode.clone(),
        reason: reason.to_string(),
    }
}

fn skip_unresolved_repo(repo: &ResolvedRepositoryDescriptor) -> RepoQuerySkippedRepository {
    RepoQuerySkippedRepository {
        display_name: repo.display_name.clone(),
        repository_id: repo.repository_id.clone(),
        access_mode: repo.access_mode.clone(),
        reason: repo
            .resolution_error
            .clone()
            .unwrap_or_else(|| repo.resolution_status.clone()),
    }
}

pub fn repo_query_text(
    repo_root: &Path,
    params: &QueryTextParams,
    cached_catalog: Option<&ResolvedRepoCatalog>,
) -> Result<RepoTextSearchResponse, RepoCatalogError> {
    let workspace = crate::discover_repository_or_fallback(repo_root);
    let catalog_owned;
    let catalog = match cached_catalog {
        Some(c) => c,
        None => {
            catalog_owned = super::resolve_repo_catalog(&workspace.root)?;
            &catalog_owned
        }
    };
    let selected = selected_repositories(catalog, params.repository_ids.as_deref());
    let target_ids = selected
        .iter()
        .filter_map(|repo| repo.repository_id.clone())
        .collect::<Vec<_>>();
    let mut trace = build_trace(
        &workspace.repository_id,
        target_ids,
        "query_text",
        "local_fs_walk",
        "local_durable",
        params.conversation_id.clone(),
    );
    let matcher = if params.regex {
        Some(
            RegexBuilder::new(&params.query)
                .case_insensitive(params.case_insensitive)
                .build()
                .map_err(|e| RepoCatalogError::InvalidRegex {
                    pattern: params.query.clone(),
                    message: e.to_string(),
                })?,
        )
    } else {
        None
    };
    let needle_lower =
        (!params.regex && params.case_insensitive).then(|| params.query.to_lowercase());
    let mut skipped = Vec::new();
    let mut hits = Vec::new();
    let mut repositories_queried = 0usize;
    let repositories_considered = selected.len();

    for repo in &selected {
        if repo.access_mode != RepoAccessMode::Local {
            skipped.push(skip_non_local_repo(
                repo,
                "remote adapter query backend not implemented for MVP",
            ));
            continue;
        }
        if repo.resolution_status != "resolved_local" {
            skipped.push(skip_unresolved_repo(repo));
            continue;
        }
        let Some(root_str) = repo.resolved_root.as_deref() else {
            continue;
        };
        let root = PathBuf::from(root_str);
        let Some(repository_id) = repo.repository_id.clone() else {
            skipped.push(skip_non_local_repo(
                repo,
                "repository_id missing after local resolution",
            ));
            continue;
        };
        repositories_queried += 1;
        let mut repo_hits = 0usize;
        let mut files_scanned = 0usize;
        let voxignore = super::VoxIgnore::load(&root);
        for entry in WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                if SKIP_DIRS.contains(&name.as_ref()) {
                    return false;
                }
                let rel = repo_relative_path(&root, e.path());
                !voxignore.is_ignored(&rel)
            })
            .filter_map(Result::ok)
        {
            if repo_hits >= params.max_matches_per_repo
                || files_scanned >= params.max_files_per_repo
            {
                break;
            }
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if !candidate_file_is_text(path, params.max_file_bytes.max(1)) {
                continue;
            }
            files_scanned += 1;
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            for (idx, line) in text.lines().enumerate() {
                let matched = if let Some(re) = matcher.as_ref() {
                    re.is_match(line)
                } else if let Some(lower) = needle_lower.as_ref() {
                    line.to_lowercase().contains(lower)
                } else {
                    line.contains(&params.query)
                };
                if matched {
                    hits.push(RepoTextMatch {
                        repository_id: repository_id.clone(),
                        display_name: repo.display_name.clone(),
                        root: root.display().to_string(),
                        path: repo_relative_path(&root, path),
                        line_number: idx + 1,
                        line_text: line.to_string(),
                    });
                    repo_hits += 1;
                    if repo_hits >= params.max_matches_per_repo {
                        break;
                    }
                }
            }
        }
    }

    trace = finalize_trace(trace);
    Ok(RepoTextSearchResponse {
        trace,
        repositories_considered,
        repositories_queried,
        result_count: hits.len(),
        skipped,
        hits,
    })
}

pub fn repo_query_file(
    repo_root: &Path,
    params: &QueryFileParams,
    cached_catalog: Option<&ResolvedRepoCatalog>,
) -> Result<RepoFileReadResponse, RepoCatalogError> {
    let workspace = crate::discover_repository_or_fallback(repo_root);
    let catalog_owned;
    let catalog = match cached_catalog {
        Some(c) => c,
        None => {
            catalog_owned = super::resolve_repo_catalog(&workspace.root)?;
            &catalog_owned
        }
    };
    let selected = selected_repositories(catalog, params.repository_ids.as_deref());
    let target_ids = selected
        .iter()
        .filter_map(|repo| repo.repository_id.clone())
        .collect::<Vec<_>>();
    let mut trace = build_trace(
        &workspace.repository_id,
        target_ids,
        "query_file",
        "local_fs_read",
        "local_durable",
        params.conversation_id.clone(),
    );
    let mut skipped = Vec::new();
    let mut files = Vec::new();
    let mut repositories_queried = 0usize;
    let repositories_considered = selected.len();

    for repo in &selected {
        if repo.access_mode != RepoAccessMode::Local {
            skipped.push(skip_non_local_repo(
                repo,
                "remote adapter file reads not implemented for MVP",
            ));
            continue;
        }
        if repo.resolution_status != "resolved_local" {
            skipped.push(skip_unresolved_repo(repo));
            continue;
        }
        let Some(root_str) = repo.resolved_root.as_deref() else {
            continue;
        };
        let root = PathBuf::from(root_str);
        let Some(repository_id) = repo.repository_id.clone() else {
            continue;
        };
        match safe_local_path(&root, &params.path) {
            Ok(path) => match std::fs::read_to_string(&path) {
                Ok(content) => {
                    repositories_queried += 1;
                    let max_bytes = params.max_bytes.max(1);
                    let mut text = content;
                    let mut truncated = false;
                    if text.len() > max_bytes {
                        text.truncate(max_bytes);
                        truncated = true;
                    }
                    files.push(RepoFileRead {
                        repository_id,
                        display_name: repo.display_name.clone(),
                        root: root.display().to_string(),
                        path: repo_relative_path(&root, &path),
                        bytes_read: text.len(),
                        truncated,
                        content: text,
                    });
                }
                Err(e) => skipped.push(RepoQuerySkippedRepository {
                    display_name: repo.display_name.clone(),
                    repository_id: Some(repository_id),
                    access_mode: repo.access_mode.clone(),
                    reason: format!("read file failed: {e}"),
                }),
            },
            Err(reason) => skipped.push(RepoQuerySkippedRepository {
                display_name: repo.display_name.clone(),
                repository_id: Some(repository_id),
                access_mode: repo.access_mode.clone(),
                reason,
            }),
        }
    }

    trace = finalize_trace(trace);
    Ok(RepoFileReadResponse {
        trace,
        repositories_considered,
        repositories_queried,
        result_count: files.len(),
        skipped,
        files,
    })
}

pub fn repo_query_history(
    repo_root: &Path,
    params: &QueryHistoryParams,
    cached_catalog: Option<&ResolvedRepoCatalog>,
) -> Result<RepoHistoryResponse, RepoCatalogError> {
    let workspace = crate::discover_repository_or_fallback(repo_root);
    let catalog_owned;
    let catalog = match cached_catalog {
        Some(c) => c,
        None => {
            catalog_owned = super::resolve_repo_catalog(&workspace.root)?;
            &catalog_owned
        }
    };
    let selected = selected_repositories(catalog, params.repository_ids.as_deref());
    let target_ids = selected
        .iter()
        .filter_map(|repo| repo.repository_id.clone())
        .collect::<Vec<_>>();
    let mut trace = build_trace(
        &workspace.repository_id,
        target_ids,
        "query_history",
        "git_log",
        "local_durable",
        params.conversation_id.clone(),
    );
    let mut skipped = Vec::new();
    let mut commits = Vec::new();
    let mut repositories_queried = 0usize;
    let repositories_considered = selected.len();

    for repo in &selected {
        if repo.access_mode != RepoAccessMode::Local {
            skipped.push(skip_non_local_repo(
                repo,
                "remote adapter history queries not implemented for MVP",
            ));
            continue;
        }
        if repo.resolution_status != "resolved_local" {
            skipped.push(skip_unresolved_repo(repo));
            continue;
        }
        let cwd = repo
            .git_root
            .as_ref()
            .or(repo.resolved_root.as_ref())
            .map(PathBuf::from);
        let Some(cwd) = cwd else {
            continue;
        };
        let Some(repository_id) = repo.repository_id.clone() else {
            continue;
        };
        let mut cmd = std::process::// vox-arch-check: allow git-exec
        Command::new("git");
        cmd.current_dir(&cwd).args([
            "log",
            "--oneline",
            "-n",
            &params.max_commits.max(1).to_string(),
        ]);
        if let Some(path) = params.path.as_deref() {
            cmd.arg("--").arg(path);
        }
        match cmd.output() {
            Ok(output) if output.status.success() => {
                repositories_queried += 1;
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    if let Some(filter) = params.contains.as_deref()
                        && !line.contains(filter)
                    {
                        continue;
                    }
                    let mut parts = line.splitn(2, ' ');
                    let commit = parts.next().unwrap_or_default().to_string();
                    let summary = parts.next().unwrap_or_default().to_string();
                    if commit.is_empty() {
                        continue;
                    }
                    commits.push(RepoHistoryEntry {
                        repository_id: repository_id.clone(),
                        display_name: repo.display_name.clone(),
                        root: cwd.display().to_string(),
                        commit,
                        summary,
                    });
                }
            }
            Ok(output) => skipped.push(RepoQuerySkippedRepository {
                display_name: repo.display_name.clone(),
                repository_id: Some(repository_id),
                access_mode: repo.access_mode.clone(),
                reason: format!(
                    "git log failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            }),
            Err(e) => skipped.push(RepoQuerySkippedRepository {
                display_name: repo.display_name.clone(),
                repository_id: Some(repository_id),
                access_mode: repo.access_mode.clone(),
                reason: format!("git not available: {e}"),
            }),
        }
    }

    trace = finalize_trace(trace);
    Ok(RepoHistoryResponse {
        trace,
        repositories_considered,
        repositories_queried,
        result_count: commits.len(),
        skipped,
        commits,
    })
}
