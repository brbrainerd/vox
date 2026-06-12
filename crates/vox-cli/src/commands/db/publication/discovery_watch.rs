//! `vox scientia discovery-watch` — commit-watcher discovery producer (Task 14 + 15).
//!
//! Scans commits since the stored `commit_watcher` cursor (or the last `--limit`
//! commits on first run) for research-worthy signals via the PURE
//! [`signals_from_commit`]. Each commit with >=1 signal becomes a DRAFT
//! publication manifest (state `"draft"`), gated through
//! `DiscoveryIntakeGate::AllowReviewSuggested`. Nothing here publishes or
//! approves. The cursor advances to HEAD only after the batch's inserts succeed.
//!
//! When an embedding provider AND a Qdrant code index are configured, a
//! Supporting-only code-uniqueness signal is folded into each candidate before
//! the gate. If either is absent the assessment is SKIPPED (never fabricated)
//! and the JSON output records `"code_uniqueness": "skipped"`.

use anyhow::{Context, Result};
use std::path::Path;
use vox_publisher::scientia_evidence::ScientiaEvidenceContext;
use vox_publisher::scientia_producers::{
    CodeKnnIndex, CodeSnippet, CommitView, assess_code_uniqueness, extract_snippets,
    signals_from_commit,
};

const PRODUCER: &str = "commit_watcher";

/// Wraps `vox_search::vector_qdrant::QdrantSemanticClient` as a [`CodeKnnIndex`].
struct QdrantCodeIndex {
    client: vox_search::vector_qdrant::QdrantSemanticClient,
    vector_name: Option<String>,
}

#[async_trait::async_trait]
impl CodeKnnIndex for QdrantCodeIndex {
    async fn max_similarity(&self, vector: &[f32]) -> Option<f64> {
        match self
            .client
            .search_vectors(vector, 1, self.vector_name.as_deref(), None)
            .await
        {
            Ok(hits) => hits.first().map(|(_, score, _)| f64::from(*score)),
            // Index unreachable → propagate absence (skip), never fabricate.
            Err(_) => None,
        }
    }
}

/// Build the Qdrant code index from search policy secrets; `None` when
/// `VOX_SEARCH_QDRANT_URL` is unconfigured (the producer then SKIPS the
/// code-uniqueness signal rather than fabricating one).
fn resolve_code_index() -> Option<QdrantCodeIndex> {
    let policy = vox_search::policy::SearchPolicy::from_env();
    let url = policy
        .qdrant_url
        .as_deref()
        .filter(|u| !u.trim().is_empty())?;
    Some(QdrantCodeIndex {
        client: vox_search::vector_qdrant::QdrantSemanticClient::new(
            url,
            policy.qdrant_collection.as_str(),
        ),
        vector_name: policy.qdrant_vector_name.clone(),
    })
}

/// Run `git` with `CREATE_NO_WINDOW` on Windows; returns stdout on success.
fn run_git(repo: &Path, args: &[&str]) -> Result<String> {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C").arg(repo).args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
        .output()
        .with_context(|| format!("spawn git {}", args.join(" ")))?;
    if !out.status.success() {
        anyhow::bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Parse `git log --numstat` output (one record per commit) into [`CommitView`]s.
///
/// Record format (most-recent first):
/// `\x1e<sha>\x1f<subject>\x1f<body...>\x1e\n<ins>\t<del>\t<path>\n...`.
/// We use record separator `\x1e` and field separator `\x1f` so newline-bearing
/// commit bodies do not corrupt parsing.
fn parse_git_log(raw: &str) -> Vec<CommitView> {
    let mut out = Vec::new();
    for record in raw.split('\x1e') {
        let record = record.trim_start_matches('\n');
        if record.trim().is_empty() {
            continue;
        }
        // Split the field-separated metadata: sha, subject, then the body which
        // may itself contain newlines (followed by numstat lines).
        let mut fields = record.splitn(3, '\x1f');
        let sha = fields.next().unwrap_or("").trim().to_string();
        if sha.is_empty() {
            continue;
        }
        let subject = fields.next().unwrap_or("").trim().to_string();
        let rest = fields.next().unwrap_or("");

        // Within `rest`, numstat lines match "<ins-or-->\t<del-or-->\t<path>".
        // Everything before the first such line is the remaining commit body.
        let mut body_lines: Vec<&str> = Vec::new();
        let mut files_changed = Vec::new();
        let mut insertions: u64 = 0;
        let mut deletions: u64 = 0;
        let mut in_numstat = false;
        for l in rest.split('\n') {
            let cols: Vec<&str> = l.splitn(3, '\t').collect();
            let is_numstat = cols.len() == 3
                && !cols[2].trim().is_empty()
                && cols[0].chars().all(|c| c.is_ascii_digit() || c == '-')
                && cols[1].chars().all(|c| c.is_ascii_digit() || c == '-')
                && !cols[0].is_empty();
            if is_numstat {
                in_numstat = true;
                insertions += cols[0].parse::<u64>().unwrap_or(0);
                deletions += cols[1].parse::<u64>().unwrap_or(0);
                files_changed.push(cols[2].trim().to_string());
            } else if !in_numstat {
                body_lines.push(l);
            }
        }
        let body = body_lines.join("\n");
        let body = body.trim();
        let message = if body.is_empty() {
            subject
        } else {
            format!("{subject}\n\n{body}")
        };

        out.push(CommitView {
            sha,
            message,
            files_changed,
            insertions,
            deletions,
        });
    }
    out
}

/// Resolve the commit range and collect [`CommitView`]s (most-recent first).
fn collect_commits(repo: &Path, cursor: Option<&str>, limit: usize) -> Result<Vec<CommitView>> {
    let fmt = "--format=%x1e%H%x1f%s%x1f%b";
    let raw = if let Some(c) = cursor.filter(|c| !c.trim().is_empty()) {
        // Verify the cursor still resolves; if not, fall back to last N.
        let range = format!("{c}..HEAD");
        match run_git(repo, &["log", &range, "--numstat", fmt]) {
            Ok(s) => s,
            Err(_) => run_git(repo, &["log", &format!("-n{limit}"), "--numstat", fmt])?,
        }
    } else {
        run_git(repo, &["log", &format!("-n{limit}"), "--numstat", fmt])?
    };
    Ok(parse_git_log(&raw))
}

/// Embedder seam alias (cache-backed LLM embedder from env).
type Embedder<'a> = crate::commands::db::publication::embedder::CachedLlmEmbedder<'a>;

/// Run the code-uniqueness assessment for one commit, returning the optional
/// signal to fold in. Skips (returns `None`) when either seam is absent, the
/// commit has no Rust snippets, or nothing could be scored.
async fn uniqueness_signal_for_commit(
    repo_root: &Path,
    c: &CommitView,
    embedder: Option<&Embedder<'_>>,
    code_index: Option<&QdrantCodeIndex>,
) -> Option<vox_publisher::scientia_evidence::DiscoverySignal> {
    let (emb, idx) = (embedder?, code_index?);
    let snippets = snippets_for_commit(repo_root, c);
    if snippets.is_empty() {
        return None;
    }
    let source_ref = format!("git:{}", c.sha);
    let assessment = assess_code_uniqueness(
        &snippets,
        emb,
        Some(idx as &dyn CodeKnnIndex),
        Some(source_ref.as_str()),
    )
    .await?;
    assessment.signal
}

/// Gather Rust snippets from a commit's changed `.rs` files (read from worktree).
fn snippets_for_commit(repo: &Path, c: &CommitView) -> Vec<CodeSnippet> {
    let mut snippets = Vec::new();
    for rel in &c.files_changed {
        if !rel.ends_with(".rs") {
            continue;
        }
        let abs = repo.join(rel);
        let Ok(src) = vox_bounded_fs::read_utf8_path_capped(&abs) else {
            continue;
        };
        snippets.extend(extract_snippets(rel, &src));
    }
    snippets
}

/// Entry point for `vox scientia discovery-watch`.
pub async fn discovery_watch(once: bool, repo: Option<&Path>, limit: usize) -> Result<()> {
    // `--once` is currently the only mode; the flag exists for forward-compat.
    let _ = once;

    let repo_root = match repo {
        Some(p) => p.to_path_buf(),
        None => vox_repository::resolve_repo_root_for_ci(),
    };
    let db = vox_db::VoxDb::connect_default().await?;

    let cursor = db.get_producer_cursor(PRODUCER).await?;
    let commits = collect_commits(&repo_root, cursor.as_deref(), limit)?;

    // HEAD sha for cursor advance (independent of how many commits were in range).
    let head = run_git(&repo_root, &["rev-parse", "HEAD"])
        .map(|s| s.trim().to_string())
        .ok();

    // Optional code-uniqueness wiring.
    let embedder = crate::commands::db::publication::embedder::CachedLlmEmbedder::from_env(&db);
    let code_index = resolve_code_index();
    let uniqueness_available = embedder.is_some() && code_index.is_some();

    let mut candidates = Vec::new();
    for c in &commits {
        let mut signals = signals_from_commit(c);
        if signals.is_empty() {
            continue;
        }

        // Fold a (Supporting-only) code-uniqueness signal when both seams exist.
        if let Some(sig) =
            uniqueness_signal_for_commit(&repo_root, c, embedder.as_ref(), code_index.as_ref())
                .await
        {
            signals.push(sig);
        }

        let signal_codes: Vec<String> = signals.iter().map(|s| s.code.clone()).collect();
        let source_ref = format!("git:{}", c.sha);
        let publication_id = format!("commit-{}", &c.sha[..c.sha.len().min(12)]);
        let subject = c.message.lines().next().unwrap_or("").trim().to_string();
        let title = if subject.is_empty() {
            format!("Discovery candidate {}", &c.sha[..c.sha.len().min(12)])
        } else {
            subject
        };
        let file_list = c
            .files_changed
            .iter()
            .map(|f| format!("- {f}"))
            .collect::<Vec<_>>()
            .join("\n");
        let body_markdown = format!("{}\n\nFiles:\n{file_list}", c.message.trim());

        let mut evidence = ScientiaEvidenceContext {
            discovery_signals: signals,
            ..Default::default()
        };
        vox_publisher::scientia_evidence::populate_candidate_context_defaults(
            Some(source_ref.as_str()),
            None,
            None,
            None,
            &mut evidence,
        );

        // Intake gate (AllowReviewSuggested): only DRAFT if it clears.
        let scientia_h =
            vox_publisher::scientia_heuristics::ScientiaHeuristics::load_from_repo_root(&repo_root);
        let rank = vox_publisher::scientia_discovery::rank_candidate_heuristics(
            &publication_id,
            Some(source_ref.as_str()),
            &evidence,
            &scientia_h,
            None,
        );
        if !vox_publisher::scientia_discovery::intake_gate_allows(
            vox_publisher::scientia_discovery::DiscoveryIntakeGate::AllowReviewSuggested,
            &rank,
        ) {
            continue;
        }

        let metadata_json = vox_publisher::scientific_metadata::build_scientia_metadata_json(
            "vox scientia discovery-watch",
            None,
            None,
            Some(&evidence),
        )
        .context("build discovery candidate metadata_json")?;

        let manifest = vox_publisher::publication::PublicationManifest {
            publication_id: publication_id.clone(),
            content_type: "scientia".to_string(),
            source_ref: Some(source_ref.clone()),
            title,
            author: PRODUCER.to_string(),
            abstract_text: None,
            body_markdown,
            citations_json: None,
            metadata_json: Some(metadata_json),
        };
        let digest = manifest.content_sha3_256();

        db.upsert_publication_manifest(vox_db::PublicationManifestParams {
            publication_id: &manifest.publication_id,
            content_type: &manifest.content_type,
            source_ref: manifest.source_ref.as_deref(),
            title: &manifest.title,
            author: &manifest.author,
            abstract_text: None,
            body_markdown: &manifest.body_markdown,
            citations_json: None,
            metadata_json: manifest.metadata_json.as_deref(),
            revision_history_json: None,
            content_sha3_256: &digest,
            state: "draft",
        })
        .await?;

        db.append_publication_status_event(
            &publication_id,
            "discovery_candidate_prepared",
            Some(
                &serde_json::json!({
                    "producer": PRODUCER,
                    "sha": c.sha,
                    "signal_codes": signal_codes,
                    "candidate_note": evidence.candidate_note,
                })
                .to_string(),
            ),
        )
        .await?;

        candidates.push(serde_json::json!({
            "publication_id": publication_id,
            "sha": c.sha,
            "signal_codes": signal_codes,
        }));
    }

    // Advance the cursor to HEAD ONLY after all inserts in the batch succeeded.
    if let Some(head_sha) = head.as_deref() {
        db.set_producer_cursor(PRODUCER, head_sha).await?;
    }

    let out = serde_json::json!({
        "scanned": commits.len(),
        "candidates_created": candidates.len(),
        "candidates": candidates,
        "code_uniqueness": if uniqueness_available { "assessed" } else { "skipped" },
        "cursor_advanced_to": head,
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_git_log_handles_numstat_and_multiline_body() {
        // Mirrors `git log --format=%x1e%H%x1f%s%x1f%b --numstat`: each commit is
        // prefixed by \x1e; numstat lines follow the body, before the next \x1e.
        let raw = "\x1eaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\x1fperf: speed up 30%\x1fbody line 1\nbody line 2\n\n12\t3\tcrates/x/src/lib.rs\n0\t4\tREADME.md\n\x1ebbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\x1fchore: tidy\x1f\n\n1\t1\tCargo.toml\n";
        let views = parse_git_log(raw);
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].sha, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert!(views[0].message.contains("perf: speed up 30%"));
        assert!(views[0].message.contains("body line 2"));
        assert_eq!(views[0].insertions, 12);
        assert_eq!(views[0].deletions, 7);
        assert_eq!(views[0].files_changed.len(), 2);
        assert_eq!(views[1].sha, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        assert_eq!(views[1].files_changed, vec!["Cargo.toml".to_string()]);
    }

    #[test]
    fn parse_git_log_empty_is_empty() {
        assert!(parse_git_log("").is_empty());
        assert!(parse_git_log("\n\n").is_empty());
    }
}
