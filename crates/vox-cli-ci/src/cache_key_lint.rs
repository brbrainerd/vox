//! `vox ci cache-key-lint` — forbid an `actions/cache` key that hashes
//! `Cargo.lock` without also keying on the Rust toolchain.
//!
//! Before this lint, 0 of 26 `actions/cache` blocks in the workflow tree
//! referenced the toolchain in their key. A cache scoped only to
//! `hashFiles(...Cargo.lock...)` survives a toolchain bump untouched: after
//! `contracts/toolchain/workspace-toolchain.v1.yaml` moves to a new Rust
//! version, every job restores object files compiled by the *old* compiler
//! under the *new* toolchain's `target/`, corrupting the build silently
//! (stale `.rlib`/`.rmeta` metadata, mismatched ABI) instead of failing
//! loudly. Anchoring the key on the toolchain forces a clean cache on every
//! bump, exactly like the OS or lockfile hash already do.
//!
//! A violation is any `actions/cache` step whose `with.key` string mentions
//! `Cargo.lock` (i.e. hashes the lockfile) without also containing the
//! substring `toolchain` anywhere in the key expression. That substring
//! check is deliberately textual, not semantic, so it accepts any of the
//! ways a site in this repo satisfies it:
//!   - `${{ steps.<id>.outputs.toolchain }}` (setup-rust's own output), or
//!   - `hashFiles('rust-toolchain.toml', 'Cargo.lock')` (for a job with no
//!     setup-rust step to draw an output from).
//!
//! Unlike the advisory guards in this crate, this one takes no `--strict`
//! flag: it always fails on a violation, mirroring `release_draft_guard` and
//! `toolchain_workflow_lint`.

use std::path::Path;

use anyhow::{Context, Result, anyhow};

const CACHE_ACTION_PREFIX: &str = "actions/cache";
const LOCKFILE_MARKER: &str = "Cargo.lock";
const TOOLCHAIN_MARKER: &str = "toolchain";

/// One violation: an `actions/cache` step in `file` named `step` whose `key`
/// hashes `Cargo.lock` but never references the toolchain.
struct Violation {
    file: String,
    step: String,
    key: String,
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

/// True when `key` hashes `Cargo.lock` but never mentions the toolchain.
fn is_untoolchained_lockfile_key(key: &str) -> bool {
    key.contains(LOCKFILE_MARKER) && !key.contains(TOOLCHAIN_MARKER)
}

/// Collects every `actions/cache` step in `doc` whose `key` hashes
/// `Cargo.lock` without referencing the toolchain, appending `file`-scoped
/// violations to `out`.
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
            let is_cache_step = step_map
                .get(serde_yaml::Value::String("uses".into()))
                .and_then(|v| v.as_str())
                .is_some_and(|s| s.starts_with(CACHE_ACTION_PREFIX));
            if !is_cache_step {
                continue;
            }
            let key = step_map
                .get(serde_yaml::Value::String("with".into()))
                .and_then(|w| w.as_mapping())
                .and_then(|w| w.get(serde_yaml::Value::String("key".into())))
                .and_then(|v| v.as_str());
            if let Some(key) = key
                && is_untoolchained_lockfile_key(key)
            {
                out.push(Violation {
                    file: file.to_string(),
                    step: step_name(step_map),
                    key: key.to_string(),
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
            "cache-key-lint OK (every Cargo.lock-hashing cache key also keys on the toolchain)"
        );
        return Ok(());
    }

    let lines: Vec<String> = violations
        .iter()
        .map(|v| format!("{}: step \"{}\" (key: {})", v.file, v.step, v.key))
        .collect();
    Err(anyhow!(
        "cache-key-lint: {} actions/cache key(s) hash Cargo.lock without keying on the \
         toolchain:\n  {}\n\
         Fix: add the toolchain to the key, e.g. `${{{{ steps.<setup-rust-id>.outputs.toolchain }}}}` \
         when the job calls ./.github/actions/setup-rust with an `id:`, or \
         `hashFiles('rust-toolchain.toml', 'Cargo.lock')` when it doesn't.",
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
    fn toolchain_output_reference_passes() {
        let yaml = "jobs:\n  \
                      build:\n    steps:\n      - name: Cache cargo\n        \
                        uses: actions/cache@v5\n        with:\n          \
                        key: ${{ runner.os }}-cargo-${{ steps.rust.outputs.toolchain }}-${{ hashFiles('Cargo.lock') }}\n";
        let violations = violations_for(yaml, "ok.yml");
        assert!(
            violations.is_empty(),
            "a key referencing steps.*.outputs.toolchain must pass"
        );
    }

    #[test]
    fn rust_toolchain_toml_hash_passes() {
        let yaml = "jobs:\n  \
                      build:\n    steps:\n      - name: Cache cargo\n        \
                        uses: actions/cache@v5\n        with:\n          \
                        key: ${{ runner.os }}-cargo-${{ hashFiles('rust-toolchain.toml', 'Cargo.lock') }}\n";
        let violations = violations_for(yaml, "ok2.yml");
        assert!(
            violations.is_empty(),
            "hashing rust-toolchain.toml alongside Cargo.lock must pass"
        );
    }

    #[test]
    fn lockfile_only_key_fails() {
        let yaml = "jobs:\n  \
                      build:\n    steps:\n      - name: Cache cargo\n        \
                        uses: actions/cache@v5\n        with:\n          \
                        key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}\n";
        let violations = violations_for(yaml, "bad.yml");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].file, "bad.yml");
    }

    #[test]
    fn key_with_no_lockfile_reference_passes() {
        // Not every cache is a cargo cache (e.g. a Playwright browser cache) --
        // this lint only fires when the key actually hashes Cargo.lock.
        let yaml = "jobs:\n  \
                      e2e:\n    steps:\n      - name: Cache Playwright browsers\n        \
                        uses: actions/cache@v5\n        with:\n          \
                        key: ms-playwright-${{ runner.os }}-${{ steps.pw_version.outputs.ver }}\n";
        let violations = violations_for(yaml, "playwright.yml");
        assert!(violations.is_empty());
    }

    #[test]
    fn non_cache_step_with_lockfile_mention_is_ignored() {
        let yaml = "jobs:\n  \
                      build:\n    steps:\n      - name: Not a cache step\n        \
                        run: echo Cargo.lock\n";
        let violations = violations_for(yaml, "not-cache.yml");
        assert!(violations.is_empty());
    }

    #[test]
    fn multiple_violations_in_one_file_are_all_reported() {
        let yaml = "jobs:\n  \
                      a:\n    steps:\n      - uses: actions/cache@v5\n        with:\n          \
                        key: ${{ hashFiles('**/Cargo.lock') }}\n  \
                      b:\n    steps:\n      - uses: actions/cache@v5\n        with:\n          \
                        key: ${{ hashFiles('Cargo.lock') }}\n";
        let violations = violations_for(yaml, "two.yml");
        assert_eq!(violations.len(), 2);
    }
}
