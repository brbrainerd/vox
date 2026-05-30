//! Claude Code transcript correlation.
//!
//! Reads `~/.claude/projects/**/*.jsonl` transcripts and correlates them with
//! a git commit by matching `cwd == repo_root` and `ts ∈ [commit_ts ± window]`.
//!
//! ## Path normalization
//!
//! Claude Code transcripts on Windows often record `cwd` in Git-Bash style
//! (`/c/Users/Owner/vox`) while a native Rust caller would pass
//! `C:\Users\Owner\vox`. We normalize both sides to a lowercase, forward-slash
//! string before comparison:
//!
//! - replace `\` with `/`
//! - if the path starts with a drive letter (`C:`), rewrite to `/c`
//! - strip trailing `/`
//! - lowercase
//!
//! This is a pragmatic string-compare rather than `fs::canonicalize`, because
//! `canonicalize` requires the path to exist on the local filesystem, which is
//! often false for transcripts produced on another machine.

use super::MeasuredCost;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Sum tokens in transcripts whose `cwd` matches `repo_root` and whose `ts`
/// falls in `[commit_ts - window, commit_ts + window]`. Returns Measured if
/// exactly one session matches, Ambiguous if more than one, Unavailable if none.
///
/// If `transcript_dir` does not exist, returns `Unavailable` (not an error).
pub fn resolve_for_commit(
    transcript_dir: &Path,
    repo_root: &Path,
    commit_ts: DateTime<Utc>,
    window: Duration,
) -> MeasuredCost {
    if !transcript_dir.exists() {
        return MeasuredCost::Unavailable;
    }

    let lo = commit_ts - window;
    let hi = commit_ts + window;
    let repo_norm = normalize_path_str(&repo_root.to_string_lossy());

    let mut jsonl_files: Vec<PathBuf> = Vec::new();
    collect_jsonl(transcript_dir, &mut jsonl_files);

    // session_id -> (input_sum, output_sum)
    let mut by_session: HashMap<String, (u64, u64)> = HashMap::new();

    for path in jsonl_files {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(val) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let Some(cwd) = val.get("cwd").and_then(|v| v.as_str()) else {
                continue;
            };
            if normalize_path_str(cwd) != repo_norm {
                continue;
            }
            let Some(ts_str) = val.get("ts").and_then(|v| v.as_str()) else {
                continue;
            };
            let Ok(ts) = DateTime::parse_from_rfc3339(ts_str) else {
                continue;
            };
            let ts_utc = ts.with_timezone(&Utc);
            if ts_utc < lo || ts_utc > hi {
                continue;
            }
            let Some(session_id) = val.get("session_id").and_then(|v| v.as_str()) else {
                continue;
            };
            let usage = val.get("usage");
            let input = usage
                .and_then(|u| u.get("input_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let output = usage
                .and_then(|u| u.get("output_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let entry = by_session.entry(session_id.to_string()).or_insert((0, 0));
            entry.0 += input;
            entry.1 += output;
        }
    }

    match by_session.len() {
        0 => MeasuredCost::Unavailable,
        1 => {
            let (session_id, (input_tokens, output_tokens)) =
                by_session.into_iter().next().expect("len==1");
            MeasuredCost::Measured {
                input_tokens,
                output_tokens,
                source: "claude-code-transcript".to_string(),
                session_id,
            }
        }
        _ => MeasuredCost::Ambiguous,
    }
}

/// Recursively collect `*.jsonl` files under `dir` into `out`.
fn collect_jsonl(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            collect_jsonl(&path, out);
        } else if ft.is_file() && path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(path);
        }
    }
}

/// Normalize a path string for cross-platform string comparison.
///
/// - `\` → `/`
/// - leading `C:` → `/c`
/// - lowercase
/// - strip trailing `/`
fn normalize_path_str(s: &str) -> String {
    let mut s = s.replace('\\', "/");
    // Drive-letter prefix like "C:/Users/..." → "/c/Users/...".
    if s.len() >= 2 {
        let bytes = s.as_bytes();
        if bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            let drive = (bytes[0] as char).to_ascii_lowercase();
            let rest = &s[2..];
            s = format!("/{}{}", drive, rest);
        }
    }
    s = s.to_ascii_lowercase();
    while s.len() > 1 && s.ends_with('/') {
        s.pop();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixture_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/transcripts")
    }

    #[test]
    fn measured_when_single_session_matches() {
        let ts = chrono::Utc.with_ymd_and_hms(2026, 5, 28, 14, 0, 15).unwrap();
        let m = resolve_for_commit(
            &fixture_dir(),
            std::path::Path::new("/c/Users/Owner/vox"),
            ts,
            Duration::minutes(2),
        );
        match m {
            MeasuredCost::Measured { input_tokens, output_tokens, session_id, .. } => {
                assert_eq!(input_tokens, 100);
                assert_eq!(output_tokens, 400);
                assert_eq!(session_id, "S1");
            }
            other => panic!("expected Measured, got {other:?}"),
        }
    }

    #[test]
    fn unavailable_when_no_window_match() {
        let ts = chrono::Utc.with_ymd_and_hms(2026, 5, 28, 16, 0, 0).unwrap();
        let m = resolve_for_commit(
            &fixture_dir(),
            std::path::Path::new("/c/Users/Owner/vox"),
            ts,
            Duration::minutes(2),
        );
        assert_eq!(m, MeasuredCost::Unavailable);
    }

    #[test]
    fn unavailable_when_cwd_mismatch() {
        // 14:05:05 is within window of S2 (cwd=/c/Users/Owner/other), but our
        // repo_root is /c/Users/Owner/vox — no rows match.
        let ts = chrono::Utc.with_ymd_and_hms(2026, 5, 28, 14, 5, 5).unwrap();
        let m = resolve_for_commit(
            &fixture_dir(),
            std::path::Path::new("/c/Users/Owner/vox"),
            ts,
            Duration::minutes(2),
        );
        assert_eq!(m, MeasuredCost::Unavailable);
    }

    #[test]
    fn unavailable_when_transcript_dir_missing() {
        let ts = chrono::Utc.with_ymd_and_hms(2026, 5, 28, 14, 0, 0).unwrap();
        let m = resolve_for_commit(
            std::path::Path::new("/nonexistent/path/that/does/not/exist-xyz"),
            std::path::Path::new("/c/Users/Owner/vox"),
            ts,
            Duration::minutes(2),
        );
        assert_eq!(m, MeasuredCost::Unavailable);
    }

    #[test]
    fn windows_native_repo_root_matches_unix_style_cwd() {
        // Caller passes Windows-style path; fixture has Git-Bash-style cwd.
        let ts = chrono::Utc.with_ymd_and_hms(2026, 5, 28, 14, 0, 15).unwrap();
        let m = resolve_for_commit(
            &fixture_dir(),
            std::path::Path::new(r"C:\Users\Owner\vox"),
            ts,
            Duration::minutes(2),
        );
        assert!(
            matches!(m, MeasuredCost::Measured { session_id, .. } if session_id == "S1"),
            "expected S1 Measured under Windows-style repo_root",
        );
    }

    #[test]
    fn normalize_path_str_handles_windows_and_unix() {
        assert_eq!(
            normalize_path_str(r"C:\Users\Owner\vox"),
            normalize_path_str("/c/Users/Owner/vox"),
        );
        assert_eq!(
            normalize_path_str("/c/Users/Owner/vox/"),
            normalize_path_str("/c/Users/Owner/vox"),
        );
    }
}
