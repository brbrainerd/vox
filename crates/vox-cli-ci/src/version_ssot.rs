//! One version, declared once, enforced everywhere.
//!
//! `[workspace.package] version` in the root `Cargo.toml` is the single source of
//! truth. Several other files restate it, and nothing checked that they agreed:
//!
//! - **9 workspace path-dependencies** carry an explicit `version = "..."`
//!   (`vox-secrets`, `vox-plugin-api`, … ). Cargo requires it on a path dependency
//!   that is also published, so it cannot simply be dropped.
//! - **3 npm packages** under `clients/` carry their own `"version"`, and two of
//!   them additionally pin `@vox/runtime-types` to the same number.
//!
//! Bumping only the workspace version leaves those behind. That is not
//! hypothetical: the first draft of `release-prepare.yml` rewrote exactly one
//! line, which would have published a 0.7.0 workspace whose own crates still
//! depended on 0.6.0.
//!
//! This module finds every restatement and reports the ones that disagree, so the
//! bump is mechanical and the drift is a CI failure rather than a release
//! surprise.

use std::path::{Path, PathBuf};

/// One place a version is restated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    pub file: PathBuf,
    pub line: usize,
    /// What that line declares.
    pub version: String,
    /// Human context, e.g. the dependency name.
    pub what: String,
}

/// A declaration that disagrees with the workspace version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drift {
    pub declaration: Declaration,
    pub expected: String,
}

/// Read `[workspace.package] version` from a root `Cargo.toml`'s text.
///
/// Scoped to that table so a dependency's `version` is never mistaken for it —
/// the same rule `version-tag-guard.yml` applies.
pub fn workspace_version(cargo_toml: &str) -> Option<String> {
    let mut in_wp = false;
    for line in cargo_toml.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_wp = t == "[workspace.package]";
            continue;
        }
        if in_wp
            && let Some(rest) = t.strip_prefix("version")
            && let Some(eq) = rest.trim_start().strip_prefix('=')
        {
            return Some(eq.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// Every `version = "..."` on a workspace path-dependency line.
///
/// These sit in `[workspace.dependencies]` as
/// `name = { path = "crates/name", version = "X" }`. Matching on the presence of
/// both `path =` and `version =` keeps registry dependencies out of scope —
/// their versions are upstream facts, not ours.
pub fn path_dependency_versions(cargo_toml: &str, file: &Path) -> Vec<Declaration> {
    cargo_toml
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("path =") && l.contains("version ="))
        .filter_map(|(i, l)| {
            let name = l.split('=').next()?.trim().to_string();
            let after = l.split("version").nth(1)?;
            let version = after
                .trim_start()
                .trim_start_matches('=')
                .trim()
                .trim_start_matches('"')
                .split('"')
                .next()?
                .to_string();
            Some(Declaration {
                file: file.to_path_buf(),
                line: i + 1,
                version,
                what: format!("path dependency `{name}`"),
            })
        })
        .collect()
}

/// The top-level `"version"` of an npm `package.json`, plus any dependency on a
/// sibling `@vox/*` package (which is pinned to the same number).
pub fn npm_versions(package_json: &str, file: &Path) -> Vec<Declaration> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    for (i, line) in package_json.lines().enumerate() {
        let t = line.trim();
        // Track nesting so the top-level "version" is distinguishable from a
        // dependency's. Counting braces is enough for the generated shape here.
        let opens = t.matches('{').count();
        let closes = t.matches('}').count();

        if depth == 1 && t.starts_with("\"version\"") {
            if let Some(v) = t.split(':').nth(1) {
                out.push(Declaration {
                    file: file.to_path_buf(),
                    line: i + 1,
                    version: v.trim().trim_matches(|c| c == '"' || c == ',').to_string(),
                    what: "package version".to_string(),
                });
            }
        } else if t.starts_with("\"@vox/")
            && let Some(v) = t.split(':').nth(1)
        {
            let name = t.split('"').nth(1).unwrap_or("@vox/?").to_string();
            let version = v.trim().trim_matches(|c| c == '"' || c == ',').to_string();
            // Range specifiers (^, ~, *) are intentional looseness, not drift.
            if version.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                out.push(Declaration {
                    file: file.to_path_buf(),
                    line: i + 1,
                    version,
                    what: format!("dependency `{name}`"),
                });
            }
        }

        depth = depth + opens - closes.min(depth);
    }
    out
}

/// Report every declaration that disagrees with `expected`.
pub fn drift(expected: &str, declarations: &[Declaration]) -> Vec<Drift> {
    declarations
        .iter()
        .filter(|d| d.version != expected)
        .map(|d| Drift {
            declaration: d.clone(),
            expected: expected.to_string(),
        })
        .collect()
}

/// The `workspace-hack` pin every member crate carries.
///
/// cargo-hakari generates `workspace-hack = { version = "0.6", path = "../workspace-hack" }`
/// into **every** member `Cargo.toml` — 113 of them here — using a TWO-component
/// requirement. `workspace-hack` inherits `version.workspace = true`, so bumping
/// the workspace to 0.7.0 invalidates all 113 pins at once and the whole build
/// stops resolving:
///
/// ```text
/// candidate versions found which didn't match: 0.7.0
/// required by package `vox-actor-runtime v0.7.0`
/// ```
///
/// This is the single largest reason a version bump "breaks everything", and it
/// is invisible to any check that only reads the root manifest.
pub fn workspace_hack_pin(cargo_toml: &str, file: &Path) -> Option<Declaration> {
    cargo_toml.lines().enumerate().find_map(|(i, l)| {
        let t = l.trim_start();
        if !t.starts_with("workspace-hack") || !t.contains("version") {
            return None;
        }
        let after = &l[l.find("version")? + "version".len()..];
        let q1 = after.find('"')?;
        let q2 = after[q1 + 1..].find('"')?;
        Some(Declaration {
            file: file.to_path_buf(),
            line: i + 1,
            version: after[q1 + 1..q1 + 1 + q2].to_string(),
            what: "workspace-hack pin".to_string(),
        })
    })
}

/// Whether `version` can be expressed as a hakari `major.minor` pin.
///
/// A prerelease or build-metadata version cannot: `major_minor("0.7.0-rc.1")` is
/// `"0.7"`, and cargo's caret semantics exclude prereleases from `^0.7`, so every
/// member pin stops matching and the workspace fails to resolve entirely. Callers
/// must refuse such a bump rather than write a tree that does not build.
pub fn is_hakari_pinnable(version: &str) -> bool {
    !version.contains('-') && !version.contains('+')
}

/// `0.7.0` -> `0.7`. hakari pins on major.minor only.
pub fn major_minor(version: &str) -> String {
    let mut it = version.split('.');
    match (it.next(), it.next()) {
        (Some(a), Some(b)) => format!("{a}.{b}"),
        _ => version.to_string(),
    }
}

/// Byte offset just past a real `version` KEY in a TOML line, or `None`.
///
/// "Real" means the token is preceded by `{`, `,`, or nothing but whitespace, and
/// followed (after whitespace) by `=`. This is what separates the key in
/// `{ path = "x", version = "1" }` from the substring in `vox-versioning` or
/// `features = ["versioned-api"]`.
fn toml_version_key_end(line: &str) -> Option<usize> {
    const KEY: &str = "version";
    let bytes = line.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = line[from..].find(KEY) {
        let start = from + rel;
        let end = start + KEY.len();
        let before_ok = line[..start]
            .rfind(|c: char| !c.is_whitespace())
            .is_none_or(|i| matches!(bytes[i], b'{' | b','));
        let after_ok = line[end..].trim_start().starts_with('=');
        if before_ok && after_ok {
            return Some(end);
        }
        from = end;
    }
    None
}

/// Rewrite every restatement of the version in `text` to `new_version`.
///
/// Line-oriented and surgical: only lines this module would *report* are
/// rewritten, so a registry dependency's version, a caret range, or any
/// unrelated `version` key is left untouched. Returns the new text and the number
/// of lines changed, so a caller can assert it changed what it expected.
pub fn rewrite(text: &str, file: &Path, new_version: &str, is_npm: bool) -> (String, usize) {
    let targets: std::collections::HashSet<usize> = if is_npm {
        npm_versions(text, file)
            .into_iter()
            .map(|d| d.line)
            .collect()
    } else {
        let mut t: std::collections::HashSet<usize> = path_dependency_versions(text, file)
            .into_iter()
            .map(|d| d.line)
            .collect();
        // The workspace version itself is the SSOT and must move too.
        let mut in_wp = false;
        for (i, line) in text.lines().enumerate() {
            let s = line.trim();
            if s.starts_with('[') {
                in_wp = s == "[workspace.package]";
                continue;
            }
            if in_wp && s.starts_with("version") && s.contains('=') {
                t.insert(i + 1);
                break;
            }
        }
        t
    };

    let mut changed = 0usize;
    let out: Vec<String> = text
        .lines()
        .enumerate()
        .map(|(i, line)| {
            // A commented-out dependency is not a declaration. Rewriting it
            // churns the diff and, worse, `path_dependency_versions` reports it
            // as drift — so a stale comment could fail the CI gate.
            if !targets.contains(&(i + 1)) || line.trim_start().starts_with('#') {
                return line.to_string();
            }
            // Replace only the first quoted semver on the line, preserving all
            // surrounding formatting (alignment, other keys, trailing commas).
            let Some(start) = line.find('"') else {
                return line.to_string();
            };
            let rest = &line[start + 1..];
            let Some(len) = rest.find('"') else {
                return line.to_string();
            };
            let old = &rest[..len];
            // Locate the VALUE, not the first quote on the line.
            //
            // TOML: `vox-x = { path = "crates/x", version = "0.6.0" }` — the first
            // quote belongs to the path, so scan from the `version` key.
            // JSON: `"version": "0.6.0",` — the key itself is quoted, so scanning
            // from `version` lands on the key's closing quote. Scan from the `:`.
            let anchor = if is_npm {
                line.find(':').map(|i| i + 1)
            } else {
                // A KEY match, not a substring search. `line.find("version")` hit
                // the `version` inside `vox-versioning = { path = ... }` and
                // rewrote the PATH; with a `features = ["versioned-api"]` entry it
                // produced unparseable TOML. Require the token to be preceded by a
                // table/list boundary and followed by `=`.
                toml_version_key_end(line)
            };
            let (start, len, old) = match anchor {
                Some(a) if a <= line.len() => {
                    let after = &line[a..];
                    match after.find('"') {
                        Some(q1) => match after[q1 + 1..].find('"') {
                            Some(q2) => (a + q1, q2, &after[q1 + 1..q1 + 1 + q2]),
                            None => (start, len, old),
                        },
                        None => (start, len, old),
                    }
                }
                _ => (start, len, old),
            };
            if old == new_version {
                return line.to_string();
            }
            changed += 1;
            format!(
                "{}{}{}",
                &line[..start + 1],
                new_version,
                &line[start + 1 + len..]
            )
        })
        .collect();

    let mut joined = out.join("\n");
    if text.ends_with('\n') {
        joined.push('\n');
    }
    (joined, changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = r#"
[workspace]
members = ["crates/*"]

[workspace.package]
version = "0.6.0"
edition = "2024"

[workspace.dependencies]
vox-secrets   = { path = "crates/vox-secrets", version = "0.6.0" }
vox-plugin-api = { path = "crates/vox-plugin-api", version = "0.5.0" }
serde         = { version = "1.0", features = ["derive"] }
"#;

    /// Regression: `line.find("version")` was a SUBSTRING search, so the
    /// `version` inside `vox-versioning` anchored the rewrite and destroyed the
    /// path; a `features = ["versioned-api"]` entry produced unparseable TOML.
    #[test]
    fn rewrite_does_not_corrupt_lines_containing_the_word_version() {
        let cases = [
            (
                r#"vox-versioning = { path = "crates/versioning-tools", version = "0.6.0" }"#,
                r#"vox-versioning = { path = "crates/versioning-tools", version = "0.9.0" }"#,
            ),
            (
                r#"vox-x = { path = "crates/x", features = ["versioned-api"], version = "0.6.0" }"#,
                r#"vox-x = { path = "crates/x", features = ["versioned-api"], version = "0.9.0" }"#,
            ),
        ];
        for (input, want) in cases {
            let (out, n) = rewrite(input, Path::new("Cargo.toml"), "0.9.0", false);
            assert_eq!(out.trim_end(), want, "corrupted: {input}");
            assert_eq!(n, 1);
        }
    }

    /// A commented-out dependency is not a declaration.
    #[test]
    fn rewrite_leaves_commented_dependencies_alone() {
        let line = r#"# vox-old = { path = "crates/old", version = "0.4.0" }"#;
        let (out, n) = rewrite(line, Path::new("Cargo.toml"), "0.9.0", false);
        assert_eq!(n, 0);
        assert_eq!(out.trim_end(), line);
    }

    /// `major_minor` truncates a prerelease to `0.7`, and cargo's caret
    /// semantics exclude prereleases from `^0.7` — so a bumped tree does not
    /// resolve at all. Refuse rather than emit a broken pin.
    #[test]
    fn major_minor_rejects_prerelease_and_build_metadata() {
        assert_eq!(major_minor("1.0.0"), "1.0");
        assert_eq!(major_minor("0.7.0"), "0.7");
        assert!(is_hakari_pinnable("0.7.0"));
        assert!(!is_hakari_pinnable("0.7.0-rc.1"));
        assert!(!is_hakari_pinnable("0.6.0+build.1917"));
    }

    #[test]
    fn workspace_hack_pin_is_found_and_is_major_minor() {
        let member = "[dependencies]\nworkspace-hack = { version = \"0.6\", path = \"../workspace-hack\" }\n";
        let d = workspace_hack_pin(member, Path::new("crates/x/Cargo.toml")).expect("pin found");
        assert_eq!(d.version, "0.6");
        assert_eq!(major_minor("0.7.0"), "0.7");
        assert_eq!(major_minor("1.12.3"), "1.12");
    }

    #[test]
    fn a_member_without_the_pin_is_not_flagged() {
        assert!(
            workspace_hack_pin("[dependencies]\nserde = \"1\"\n", Path::new("c/Cargo.toml"))
                .is_none()
        );
        // The `{ workspace = true }` form carries no version and needs no rewrite.
        assert!(
            workspace_hack_pin(
                "workspace-hack = { workspace = true }\n",
                Path::new("c/Cargo.toml")
            )
            .is_none()
        );
    }

    #[test]
    fn rewrite_moves_workspace_and_path_deps_but_not_registry_deps() {
        let (out, n) = rewrite(ROOT, Path::new("Cargo.toml"), "0.7.0", false);
        assert_eq!(n, 3, "workspace version + two path deps");
        assert!(
            out.contains(r#"vox-secrets   = { path = "crates/vox-secrets", version = "0.7.0" }"#)
        );
        assert!(
            out.contains(
                r#"vox-plugin-api = { path = "crates/vox-plugin-api", version = "0.7.0" }"#
            )
        );
        // serde must be untouched.
        assert!(out.contains(r#"serde         = { version = "1.0", features = ["derive"] }"#));
        assert!(workspace_version(&out).as_deref() == Some("0.7.0"));
        assert!(
            drift(
                "0.7.0",
                &path_dependency_versions(&out, Path::new("Cargo.toml"))
            )
            .is_empty()
        );
    }

    #[test]
    fn rewrite_moves_npm_version_and_sibling_pin_only() {
        let (out, n) = rewrite(PKG, Path::new("p.json"), "0.7.0", true);
        assert_eq!(n, 2);
        assert!(out.contains(r#""version": "0.7.0""#));
        assert!(out.contains(r#""@vox/runtime-types": "0.7.0""#));
        assert!(
            out.contains(r#""react": "^19.0.0""#),
            "third-party range must not move"
        );
    }

    #[test]
    fn rewrite_is_idempotent() {
        let (once, n1) = rewrite(ROOT, Path::new("Cargo.toml"), "0.7.0", false);
        let (twice, n2) = rewrite(&once, Path::new("Cargo.toml"), "0.7.0", false);
        assert_eq!(n2, 0, "a second run must change nothing");
        assert_eq!(once, twice);
        assert!(n1 > 0);
    }

    #[test]
    fn workspace_version_ignores_dependency_versions() {
        assert_eq!(workspace_version(ROOT).as_deref(), Some("0.6.0"));
    }

    #[test]
    fn path_dependencies_are_collected_and_registry_deps_are_not() {
        let d = path_dependency_versions(ROOT, Path::new("Cargo.toml"));
        assert_eq!(
            d.len(),
            2,
            "serde is a registry dep and must be ignored: {d:?}"
        );
        assert!(d.iter().any(|x| x.what.contains("vox-secrets")));
    }

    #[test]
    fn drift_names_the_lagging_declaration() {
        let d = path_dependency_versions(ROOT, Path::new("Cargo.toml"));
        let drifted = drift("0.6.0", &d);
        assert_eq!(drifted.len(), 1);
        assert!(drifted[0].declaration.what.contains("vox-plugin-api"));
        assert_eq!(drifted[0].declaration.version, "0.5.0");
    }

    const PKG: &str = r#"{
  "name": "@vox/runtime-web",
  "version": "0.6.0",
  "dependencies": {
    "@vox/runtime-types": "0.6.0",
    "react": "^19.0.0"
  }
}"#;

    #[test]
    fn npm_top_level_and_sibling_pin_are_both_found() {
        let d = npm_versions(PKG, Path::new("clients/runtime-web/package.json"));
        assert_eq!(
            d.len(),
            2,
            "want the package version and the @vox pin: {d:?}"
        );
        assert!(d.iter().any(|x| x.what == "package version"));
        assert!(d.iter().any(|x| x.what.contains("@vox/runtime-types")));
        // A caret range is deliberate looseness on a third-party dep, not drift.
        assert!(!d.iter().any(|x| x.what.contains("react")));
    }

    #[test]
    fn npm_drift_is_detected() {
        let stale = PKG.replace(
            r#""@vox/runtime-types": "0.6.0""#,
            r#""@vox/runtime-types": "0.5.0""#,
        );
        let d = npm_versions(&stale, Path::new("p.json"));
        assert_eq!(drift("0.6.0", &d).len(), 1);
    }

    #[test]
    fn a_consistent_workspace_reports_no_drift() {
        let clean = ROOT.replace(
            r#"vox-plugin-api = { path = "crates/vox-plugin-api", version = "0.5.0" }"#,
            r#"vox-plugin-api = { path = "crates/vox-plugin-api", version = "0.6.0" }"#,
        );
        let d = path_dependency_versions(&clean, Path::new("Cargo.toml"));
        assert!(drift("0.6.0", &d).is_empty());
    }
}
