//! `vox ci release-draft-guard` — require every workflow step whose `uses:`
//! starts with `softprops/action-gh-release` to set `with.draft` to the
//! boolean `true`. `action-gh-release` defaults to a published, public,
//! `latest` release when `draft:` is absent — this repo has repeatedly been
//! told not to publish a genuinely public release, so a missing or falsy
//! `draft` is a hard failure, not an advisory warning.
//!
//! It also scans `run:` step script bodies for a `gh release create`
//! invocation and requires `--draft` on the same logical command line
//! (joining `\`-continued lines first). The action-based check alone once
//! missed this: a scripted `gh release create` is a second, independent way
//! to auto-publish a genuinely public release, and this repo has already
//! shipped one nightly-tag workflow that fanned out to an unguarded
//! `action-gh-release` step (see `docs/src/architecture/nightly-builds-ssot.md`
//! "History: the removed `nightly-tag.yml`"). A comment line mentioning `gh
//! release create` (e.g. explaining another step's safety invariant) is not
//! itself a violation.
//!
//! Unlike the advisory guards in this crate, this one takes no `--strict`
//! flag: it always fails on a violation.

use std::path::Path;

use anyhow::{Context, Result, anyhow};

const RELEASE_ACTION_PREFIX: &str = "softprops/action-gh-release";
const GH_RELEASE_CREATE: &str = "gh release create";

/// One violation: a `uses: softprops/action-gh-release*` step in `file` named
/// `step` that does not set `with.draft: true`.
struct Violation {
    file: String,
    step: String,
}

/// True only when `with.draft` is present and is the YAML boolean `true`
/// (not the string `"true"`, not absent, not `false`). serde_yaml (YAML 1.1)
/// distinguishes `Value::Bool` from `Value::String` — mirrors the same care
/// `workflow_concurrency_guard::has_cancelling_concurrency` takes for
/// `cancel-in-progress: true`.
fn step_has_draft_true(step: &serde_yaml::Mapping) -> bool {
    step.get(serde_yaml::Value::String("with".into()))
        .and_then(|w| w.as_mapping())
        .and_then(|w| w.get(serde_yaml::Value::String("draft".into())))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Joins shell `\`-continued lines in a `run:` script into logical command
/// lines, so a `gh release create ... \` spread across several physical
/// lines is scanned as one command. A trailing `\` (optionally followed by
/// trailing whitespace) merges the current physical line with the next.
fn join_continuations(script: &str) -> Vec<String> {
    let mut logical = Vec::new();
    let mut buf = String::new();
    for raw_line in script.lines() {
        let trimmed_end = raw_line.trim_end();
        if let Some(stripped) = trimmed_end.strip_suffix('\\') {
            if !buf.is_empty() {
                buf.push(' ');
            }
            buf.push_str(stripped.trim_end());
        } else {
            if !buf.is_empty() {
                buf.push(' ');
                buf.push_str(trimmed_end.trim());
                logical.push(std::mem::take(&mut buf));
            } else {
                logical.push(trimmed_end.to_string());
            }
        }
    }
    if !buf.is_empty() {
        logical.push(buf);
    }
    logical
}

/// Returns `true` if `logical_line` is a `gh release create` invocation that
/// lacks `--draft` on the same (continuation-joined) logical line. Comment
/// lines (whose trimmed text starts with `#`) never count, even if they
/// happen to mention `gh release create` while explaining another step.
fn is_undrafted_release_create(logical_line: &str) -> bool {
    let trimmed = logical_line.trim_start();
    if trimmed.starts_with('#') {
        return false;
    }
    trimmed.contains(GH_RELEASE_CREATE) && !logical_line.contains("--draft")
}

/// Scans a `run:` step's script body for an unguarded `gh release create`.
/// Returns the first offending logical line, if any, for use in the
/// violation message.
fn find_undrafted_release_create(script: &str) -> Option<String> {
    join_continuations(script)
        .into_iter()
        .find(|line| is_undrafted_release_create(line))
}

/// Step display name: prefers the `name:` field, falls back to `uses:`.
fn step_name(step: &serde_yaml::Mapping) -> String {
    step.get(serde_yaml::Value::String("name".into()))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            step.get(serde_yaml::Value::String("uses".into()))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "<unnamed step>".to_string())
}

/// Collects every `softprops/action-gh-release` step in `doc` that lacks
/// `draft: true`, appending `file`-scoped violations to `out`.
fn check_doc(doc: &serde_yaml::Value, file: &str, out: &mut Vec<Violation>) {
    let Some(jobs) = doc
        .as_mapping()
        .and_then(|m| m.get(serde_yaml::Value::String("jobs".into())))
        .and_then(|j| j.as_mapping())
    else {
        return;
    };
    for (_job_name, job) in jobs {
        let Some(steps) = job
            .as_mapping()
            .and_then(|j| j.get(serde_yaml::Value::String("steps".into())))
            .and_then(|s| s.as_sequence())
        else {
            continue;
        };
        for step in steps {
            let Some(step_map) = step.as_mapping() else {
                continue;
            };
            let is_release_step = step_map
                .get(serde_yaml::Value::String("uses".into()))
                .and_then(|v| v.as_str())
                .is_some_and(|s| s.starts_with(RELEASE_ACTION_PREFIX));
            if is_release_step && !step_has_draft_true(step_map) {
                out.push(Violation {
                    file: file.to_string(),
                    step: step_name(step_map),
                });
            }

            let run_violation = step_map
                .get(serde_yaml::Value::String("run".into()))
                .and_then(|v| v.as_str())
                .and_then(find_undrafted_release_create);
            if let Some(offending_line) = run_violation {
                out.push(Violation {
                    file: file.to_string(),
                    step: format!("{} (run: `{}`)", step_name(step_map), offending_line.trim()),
                });
            }
        }
    }
}

pub fn run(repo_root: &Path) -> Result<()> {
    let wf_dir = repo_root.join(".github").join("workflows");
    let mut entries: Vec<_> = std::fs::read_dir(&wf_dir)
        .with_context(|| format!("read {}", wf_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "yml" || x == "yaml"))
        .collect();
    entries.sort();

    let mut violations = Vec::new();
    for path in entries {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        check_doc(&doc, &name, &mut violations);
    }

    if violations.is_empty() {
        println!(
            "release-draft-guard OK (no undrafted softprops/action-gh-release step or `gh release create` invocation)"
        );
        return Ok(());
    }

    let lines: Vec<String> = violations
        .iter()
        .map(|v| format!("{}: step \"{}\"", v.file, v.step))
        .collect();
    Err(anyhow!(
        "release-draft-guard: {} undrafted release-publishing step(s):\n  {}\n\
         Fix: add `draft: true` (and `prerelease: true`) to the `action-gh-release` step's \
         `with:`, or `--draft` to the `gh release create` invocation. \
         This repo never publishes a genuinely public GitHub release automatically; \
         drafts are promoted by hand.",
        violations.len(),
        lines.join("\n  ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn violations_for(yaml: &str, file: &str) -> Vec<Violation> {
        let doc: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let mut out = Vec::new();
        check_doc(&doc, file, &mut out);
        out
    }

    #[test]
    fn draft_true_passes() {
        let yaml = "jobs:\n  \
                      release:\n    steps:\n      - name: Create release\n        \
                        uses: softprops/action-gh-release@v3\n        with:\n          draft: true\n";
        let violations = violations_for(yaml, "ok.yml");
        assert!(violations.is_empty(), "draft: true must pass");
    }

    #[test]
    fn missing_draft_key_fails() {
        let yaml = "jobs:\n  \
                      release:\n    steps:\n      - name: Create release\n        \
                        uses: softprops/action-gh-release@v3\n        with:\n          files: x\n";
        let violations = violations_for(yaml, "missing.yml");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].file, "missing.yml");
        assert_eq!(violations[0].step, "Create release");
    }

    #[test]
    fn draft_false_fails() {
        let yaml = "jobs:\n  \
                      release:\n    steps:\n      - name: Create release\n        \
                        uses: softprops/action-gh-release@v3\n        with:\n          draft: false\n";
        let violations = violations_for(yaml, "false.yml");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn no_release_action_passes() {
        let yaml = "jobs:\n  \
                      build:\n    steps:\n      - name: Build\n        run: echo hi\n";
        let violations = violations_for(yaml, "none.yml");
        assert!(violations.is_empty());
    }

    #[test]
    fn draft_string_true_fails() {
        // "true" as a YAML string (quoted) is not the boolean true.
        let yaml = "jobs:\n  \
                      release:\n    steps:\n      - name: Create release\n        \
                        uses: softprops/action-gh-release@v3\n        with:\n          draft: \"true\"\n";
        let violations = violations_for(yaml, "string.yml");
        assert_eq!(
            violations.len(),
            1,
            "quoted \"true\" string must not satisfy the bool check"
        );
    }

    #[test]
    fn run_gh_release_create_with_draft_passes() {
        let yaml = r#"
jobs:
  publish:
    steps:
      - name: Create draft nightly release
        run: |
          gh release create x --draft --prerelease
"#;
        let violations = violations_for(yaml, "run-ok.yml");
        assert!(
            violations.is_empty(),
            "gh release create with --draft on the same line must pass"
        );
    }

    #[test]
    fn run_gh_release_create_without_draft_fails() {
        let yaml = r#"
jobs:
  publish:
    steps:
      - name: Create release
        run: |
          gh release create x
"#;
        let violations = violations_for(yaml, "run-missing.yml");
        assert_eq!(
            violations.len(),
            1,
            "gh release create without --draft must fail"
        );
        assert_eq!(violations[0].file, "run-missing.yml");
    }

    #[test]
    fn run_gh_release_view_and_upload_do_not_trip() {
        let yaml = r#"
jobs:
  publish:
    steps:
      - name: Update existing release
        run: |
          gh release view "$TAG" >/dev/null 2>&1
          gh release upload "$TAG" file.txt --clobber
"#;
        let violations = violations_for(yaml, "view-upload.yml");
        assert!(
            violations.is_empty(),
            "gh release view/upload (no `create`) must never trip the guard"
        );
    }

    #[test]
    fn run_gh_release_create_split_across_continuation_fails() {
        let yaml = r#"
jobs:
  publish:
    steps:
      - name: Create release
        run: |
          gh release create "$TAG" "${files[@]}" \
            --title "Nightly" \
            --notes "auto"
"#;
        let violations = violations_for(yaml, "continuation.yml");
        assert_eq!(
            violations.len(),
            1,
            "a `gh release create` split across `\\` continuations without \
             --draft anywhere on the joined command must still be caught"
        );
    }

    #[test]
    fn run_gh_release_create_continuation_with_draft_passes() {
        let yaml = r#"
jobs:
  publish:
    steps:
      - name: Create release
        run: |
          gh release create "$TAG" "${files[@]}" \
            --draft --prerelease \
            --title "Nightly"
"#;
        let violations = violations_for(yaml, "continuation-ok.yml");
        assert!(
            violations.is_empty(),
            "--draft on a continuation line of the same logical command must satisfy the guard"
        );
    }

    #[test]
    fn comment_mentioning_gh_release_create_does_not_trip() {
        let yaml = r#"
jobs:
  publish:
    steps:
      - name: Create draft nightly release
        run: |
          # SAFETY: the single release-creating step is
          # `gh release create ... --draft --prerelease`, matched by an
          # idempotent "update if it already exists" branch.
          gh release create x --draft --prerelease
"#;
        let violations = violations_for(yaml, "comment.yml");
        assert!(
            violations.is_empty(),
            "a comment line mentioning gh release create must not itself count as a violation"
        );
    }
}
