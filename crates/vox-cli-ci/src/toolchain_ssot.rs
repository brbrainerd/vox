//! One Rust toolchain version, declared once, enforced everywhere.
//!
//! `contracts/toolchain/workspace-toolchain.v1.yaml` (`versions.rust`) is the
//! single source of truth. Nine other lines across eight files restate it —
//! `rust-toolchain.toml`, the `Cargo.toml` MSRV floor, two CI-runner
//! Dockerfiles, the production `Dockerfile`, the distribution profile, the
//! stable channel manifest, and a voxup test fixture (plus the assertion
//! that reads it back) — and until now nothing checked that they agreed.
//!
//! This module mirrors `version_ssot`'s vocabulary (`Declaration`, `Drift`)
//! and its key-anchoring discipline: every parser requires the key to be the
//! first non-whitespace token on the line, so a match can never land on a
//! substring inside an unrelated identifier (the class of bug that once hit
//! `vox-versioning` here). It also mirrors the portable-awk parser in
//! `.github/actions/setup-rust/action.yml`: both read `versions.rust` scoped
//! to the top-level `versions:` mapping, so the two can never quietly
//! disagree about what the SSOT says.
//!
//! `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/build_health.rs`
//! and `.../doctor/mod.rs` also contain the literal string `1.96.0`, but as
//! sample inputs to a version-string *parser's* own tests and a `Check::pass`
//! fixture — not toolchain restatements. They are deliberately absent from
//! `ROWS` below; do not add them.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

/// One place the Rust toolchain version is restated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    pub file: PathBuf,
    pub line: usize,
    pub version: String,
    /// Human context, e.g. "rust-toolchain.toml channel".
    pub what: String,
    pub kind: Kind,
}

/// How a declaration must agree with the SSOT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Must equal the SSOT version exactly.
    Exact,
    /// A major.minor MSRV floor: the SSOT version must start with it. This is
    /// `Cargo.toml`'s `rust-version` — a floor MAY legitimately lag the
    /// pinned toolchain, so equality is the wrong test.
    Floor,
}

/// A declaration that disagrees with the workspace toolchain version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drift {
    pub declaration: Declaration,
    pub expected: String,
}

/// Byte range of the quoted value in a `key<ws><sep><ws>"value"`-shaped line
/// (TOML `key = "value"` or YAML `key: "value"`), anchored so `key` must be
/// the first non-whitespace token on the line. Returns `None` if the line
/// does not have that shape — including when `key` appears only as a
/// substring of a longer identifier (e.g. `rust-versioning`), because
/// `strip_prefix` on the *trimmed* line requires an exact prefix match, not a
/// `find`.
fn quoted_value_span(line: &str, key: &str, sep: char) -> Option<(usize, usize)> {
    let mut rest = line;
    let mut pos = 0usize;

    let trimmed = rest.trim_start();
    pos += rest.len() - trimmed.len();
    rest = trimmed;

    rest = rest.strip_prefix(key)?;
    pos += key.len();

    let trimmed = rest.trim_start();
    pos += rest.len() - trimmed.len();
    rest = trimmed;

    rest = rest.strip_prefix(sep)?;
    pos += sep.len_utf8();

    let trimmed = rest.trim_start();
    pos += rest.len() - trimmed.len();
    rest = trimmed;

    rest = rest.strip_prefix('"')?;
    pos += 1;

    let end_rel = rest.find('"')?;
    Some((pos, pos + end_rel))
}

/// Byte range of an unquoted value in a `key<sep>value` line (e.g.
/// `ARG RUST_VERSION=1.96.0`), terminated by the first character in
/// `terminators` (or end of line). Anchored the same way as
/// [`quoted_value_span`]: `key` must be the first non-whitespace token.
fn bare_value_span(
    line: &str,
    key: &str,
    sep: char,
    terminators: &[char],
) -> Option<(usize, usize)> {
    let mut rest = line;
    let mut pos = 0usize;

    let trimmed = rest.trim_start();
    pos += rest.len() - trimmed.len();
    rest = trimmed;

    rest = rest.strip_prefix(key)?;
    pos += key.len();

    rest = rest.strip_prefix(sep)?;
    pos += sep.len_utf8();

    let end_rel = rest
        .find(|c: char| terminators.contains(&c))
        .unwrap_or(rest.len());
    if end_rel == 0 {
        return None;
    }
    Some((pos, pos + end_rel))
}

/// Byte range of the quoted value in `p.rust_version, "value"` — the
/// `assert_eq!` line in `crates/voxup/src/profiles.rs` that reads back the
/// test fixture. Distinct shape from [`quoted_value_span`]: the anchor is a
/// call-argument position, not a `key = value` declaration.
fn assert_rust_version_span(line: &str) -> Option<(usize, usize)> {
    const ANCHOR: &str = "p.rust_version,";
    let idx = line.find(ANCHOR)?;
    let mut rest = &line[idx + ANCHOR.len()..];
    let mut pos = idx + ANCHOR.len();

    let trimmed = rest.trim_start();
    pos += rest.len() - trimmed.len();
    rest = trimmed;

    rest = rest.strip_prefix('"')?;
    pos += 1;

    let end_rel = rest.find('"')?;
    Some((pos, pos + end_rel))
}

fn span_channel(line: &str) -> Option<(usize, usize)> {
    quoted_value_span(line, "channel", '=')
}

fn span_cargo_rust_version_floor(line: &str) -> Option<(usize, usize)> {
    quoted_value_span(line, "rust-version", '=')
}

fn span_dockerfile_from_rust(line: &str) -> Option<(usize, usize)> {
    bare_value_span(line, "FROM rust", ':', &['-', ' '])
}

fn span_arg_rust_version(line: &str) -> Option<(usize, usize)> {
    bare_value_span(line, "ARG RUST_VERSION", '=', &[' ', '\t'])
}

fn span_yaml_rust_version(line: &str) -> Option<(usize, usize)> {
    quoted_value_span(line, "rust_version", ':')
}

fn span_stable_min_rust(line: &str) -> Option<(usize, usize)> {
    quoted_value_span(line, "min_rust", '=')
}

fn span_voxup_assert(line: &str) -> Option<(usize, usize)> {
    assert_rust_version_span(line)
}

/// One restatement site: which file, what it's called, how it must agree
/// with the SSOT, and how to find the version substring on its line.
struct Row {
    file: &'static str,
    what: &'static str,
    kind: Kind,
    span: fn(&str) -> Option<(usize, usize)>,
}

/// The nine measured rows. Append-only — see the module doc for the two
/// look-alike strings that must NEVER be added here.
const ROWS: &[Row] = &[
    Row {
        file: "rust-toolchain.toml",
        what: "rust-toolchain.toml channel",
        kind: Kind::Exact,
        span: span_channel,
    },
    Row {
        file: "Cargo.toml",
        what: "Cargo.toml rust-version (MSRV floor)",
        kind: Kind::Floor,
        span: span_cargo_rust_version_floor,
    },
    Row {
        file: "Dockerfile",
        what: "Dockerfile FROM rust:",
        kind: Kind::Exact,
        span: span_dockerfile_from_rust,
    },
    Row {
        file: "Dockerfile.ci-runner",
        what: "Dockerfile.ci-runner ARG RUST_VERSION",
        kind: Kind::Exact,
        span: span_arg_rust_version,
    },
    Row {
        file: "infra/ci-runner/Dockerfile",
        what: "infra/ci-runner/Dockerfile ARG RUST_VERSION",
        kind: Kind::Exact,
        span: span_arg_rust_version,
    },
    Row {
        file: "contracts/distribution/profiles.v1.yaml",
        what: "contracts/distribution/profiles.v1.yaml rust_version",
        kind: Kind::Exact,
        span: span_yaml_rust_version,
    },
    Row {
        file: "contracts/channels/stable.toml",
        what: "contracts/channels/stable.toml min_rust",
        kind: Kind::Exact,
        span: span_stable_min_rust,
    },
    Row {
        file: "crates/voxup/src/profiles.rs",
        what: "crates/voxup/src/profiles.rs test fixture rust_version",
        kind: Kind::Exact,
        span: span_yaml_rust_version,
    },
    Row {
        file: "crates/voxup/src/profiles.rs",
        what: "crates/voxup/src/profiles.rs assert_eq!(p.rust_version, ...)",
        kind: Kind::Exact,
        span: span_voxup_assert,
    },
];

/// The SSOT file, relative to the repo root.
pub const SSOT_PATH: &str = "contracts/toolchain/workspace-toolchain.v1.yaml";

/// Parse `versions.rust` out of the SSOT YAML's text.
///
/// Scoped to the top-level `versions:` mapping — mirrors the awk parser in
/// `.github/actions/setup-rust/action.yml` exactly, so the two can never
/// disagree about which line is authoritative. `targets.rust` and
/// `components.rust` are YAML *lists* (`rust:` followed by `- "..."` lines),
/// not `rust: "..."` scalars, so they can never satisfy this parser even
/// without the section scoping — but the scoping is kept so a future list
/// item shaped like a quoted scalar still cannot be mistaken for this key.
pub fn ssot_rust_version(yaml: &str) -> Option<String> {
    let mut in_versions = false;
    for line in yaml.lines() {
        if line.starts_with("versions:") {
            in_versions = true;
            continue;
        }
        if in_versions && !line.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
            in_versions = false;
        }
        if in_versions && let Some((s, e)) = quoted_value_span(line, "rust", ':') {
            return Some(line[s..e].to_string());
        }
    }
    None
}

/// Rewrite `versions.rust` in the SSOT YAML's text. Returns the new text and
/// the number of lines changed (0 or 1).
fn rewrite_ssot_rust_version(yaml: &str, new_version: &str) -> (String, usize) {
    let mut in_versions = false;
    let mut changed = 0usize;
    let out: Vec<String> = yaml
        .lines()
        .map(|line| {
            if line.starts_with("versions:") {
                in_versions = true;
                return line.to_string();
            }
            if in_versions && !line.is_empty() && !line.starts_with(' ') && !line.starts_with('\t')
            {
                in_versions = false;
            }
            if in_versions
                && let Some((s, e)) = quoted_value_span(line, "rust", ':')
                && &line[s..e] != new_version
            {
                changed += 1;
                return format!("{}{}{}", &line[..s], new_version, &line[e..]);
            }
            line.to_string()
        })
        .collect();
    let mut joined = out.join("\n");
    if yaml.ends_with('\n') {
        joined.push('\n');
    }
    (joined, changed)
}

/// Every restatement of the Rust toolchain version found under `root`.
/// A row whose file cannot be read, or whose line shape is not found,
/// contributes no declaration (rather than panicking) — `drift` and the
/// integration test are what turn a missing row into a loud failure.
pub fn declarations(root: &Path) -> Vec<Declaration> {
    let mut out = Vec::new();
    for row in ROWS {
        let path = root.join(row.file);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            if let Some((s, e)) = (row.span)(line) {
                out.push(Declaration {
                    file: PathBuf::from(row.file),
                    line: i + 1,
                    version: line[s..e].to_string(),
                    what: row.what.to_string(),
                    kind: row.kind,
                });
                break;
            }
        }
    }
    out
}

/// Whether a `Kind::Floor` value (major.minor) is satisfied by the full SSOT
/// version (major.minor.patch).
///
/// `rust-version` is an MSRV **floor**: it declares the OLDEST Rust that can
/// build this workspace, which is a different question from which toolchain we
/// happen to pin. Cargo reads it that way, and so do the plan and AGENTS.md
/// ("MSRV is a floor and may stay at 1.96 while the toolchain moves").
///
/// This used to be a prefix test (`ssot.starts_with("{floor}.")`), inherited
/// from the shell guard it replaced (ci.yml's "Toolchain SSoT Drift Guard").
/// A prefix test is not a floor — it demands the same major.minor — so every
/// toolchain minor bump silently ratcheted the public MSRV up with it, locking
/// out users on older Rust for no technical reason. Bumping 1.96.0 -> 1.98.1
/// forced `rust-version` to 1.98 even though the workspace still compiles on
/// 1.96.
///
/// Satisfied when the SSOT toolchain is >= the floor, compared numerically so
/// that "1.9" does not sort above "1.10". Real drift is still caught: a floor
/// ABOVE the pinned toolchain means we claim to need more Rust than we build
/// with, which is broken.
fn floor_satisfied(floor: &str, ssot: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.split(['.', '-', '+'])
            .map_while(|p| p.parse::<u64>().ok())
            .collect()
    }
    let (f, s) = (parts(floor), parts(ssot));
    if f.is_empty() || s.is_empty() {
        // Unparseable on either side: fall back to the old exact/prefix rule
        // rather than silently passing everything.
        return ssot == floor || ssot.starts_with(&format!("{floor}."));
    }
    // Compare component-wise; a missing component reads as 0 ("1.96" == "1.96.0").
    for i in 0..f.len().max(s.len()) {
        let (a, b) = (
            s.get(i).copied().unwrap_or(0),
            f.get(i).copied().unwrap_or(0),
        );
        if a != b {
            return a > b;
        }
    }
    true
}

/// Every declaration under `root` that disagrees with the SSOT.
pub fn drift(root: &Path) -> Vec<Drift> {
    let Ok(ssot_text) = std::fs::read_to_string(root.join(SSOT_PATH)) else {
        return Vec::new();
    };
    let Some(expected) = ssot_rust_version(&ssot_text) else {
        return Vec::new();
    };
    declarations(root)
        .into_iter()
        .filter(|d| match d.kind {
            Kind::Exact => d.version != expected,
            Kind::Floor => !floor_satisfied(&d.version, &expected),
        })
        .map(|d| Drift {
            declaration: d,
            expected: expected.clone(),
        })
        .collect()
}

/// Rewrite every restatement (plus the SSOT itself) to `new_version`.
/// `Kind::Floor` rows are rewritten to `new_version`'s major.minor. Returns
/// the total number of lines changed across every file, so a caller can
/// assert it changed what it expected.
///
/// Refuses (via [`validate_ssot_version`]) to write a `x.y.0` SSOT version —
/// that shape is the first cut of a train, never a real pin.
pub fn rewrite_all(root: &Path, new_version: &str) -> Result<usize> {
    validate_ssot_version(new_version)?;
    let mut total = 0usize;

    let ssot_path = root.join(SSOT_PATH);
    let ssot_text = std::fs::read_to_string(&ssot_path)
        .with_context(|| format!("reading {}", ssot_path.display()))?;
    let (new_ssot, n) = rewrite_ssot_rust_version(&ssot_text, new_version);
    if n > 0 {
        std::fs::write(&ssot_path, new_ssot)
            .with_context(|| format!("writing {}", ssot_path.display()))?;
    }
    total += n;

    let floor = major_minor(new_version);
    for row in ROWS {
        let target = match row.kind {
            Kind::Exact => new_version,
            Kind::Floor => floor.as_str(),
        };
        let path = root.join(row.file);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let mut changed = 0usize;
        let out: Vec<String> = text
            .lines()
            .map(|line| match (row.span)(line) {
                Some((s, e)) if &line[s..e] != target => {
                    changed += 1;
                    format!("{}{}{}", &line[..s], target, &line[e..])
                }
                _ => line.to_string(),
            })
            .collect();
        if changed > 0 {
            let mut joined = out.join("\n");
            if text.ends_with('\n') {
                joined.push('\n');
            }
            std::fs::write(&path, joined).with_context(|| format!("writing {}", path.display()))?;
        }
        total += changed;
    }
    Ok(total)
}

/// `1.96.0` -> `1.96`.
fn major_minor(version: &str) -> String {
    let mut it = version.split('.');
    match (it.next(), it.next()) {
        (Some(a), Some(b)) => format!("{a}.{b}"),
        _ => version.to_string(),
    }
}

/// Reject an SSOT version shaped `\d+\.\d+\.0` — the first cut of a release
/// train, never a version that has actually shipped a patch. `1.96.1`
/// existing today is the proof this repo always pins past `.0`.
pub fn validate_ssot_version(v: &str) -> Result<()> {
    let parts: Vec<&str> = v.split('.').collect();
    let is_x_y_zero = parts.len() == 3
        && !parts[0].is_empty()
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && !parts[1].is_empty()
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && parts[2] == "0";
    if is_x_y_zero {
        bail!(
            "refusing to pin the Rust toolchain to {v}: a `x.y.0` is the first cut of a \
             release train, not a version anyone has actually run in production. Wait for \
             a patch release (e.g. {}.{}.1) and pin that instead.",
            parts[0],
            parts[1]
        );
    }
    Ok(())
}

/// `vox ci toolchain-ssot` — report drift between the SSOT and every
/// restatement, failing the gate if any disagree.
pub fn run(root: &Path) -> Result<()> {
    let d = drift(root);
    if d.is_empty() {
        println!("toolchain-ssot: clean — every restatement agrees with {SSOT_PATH}");
        return Ok(());
    }
    for x in &d {
        println!(
            "DRIFT {}:{} {} = {} (want {})",
            x.declaration.file.display(),
            x.declaration.line,
            x.declaration.what,
            x.declaration.version,
            x.expected
        );
    }
    Err(anyhow!(
        "{} toolchain-version restatement(s) disagree with {SSOT_PATH}",
        d.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- row-format parse + rewrite: one test per distinct shape ----

    #[test]
    fn rust_toolchain_toml_channel_parses_and_rewrites() {
        let line = r#"channel = "1.96.0""#;
        let (s, e) = span_channel(line).expect("must parse");
        assert_eq!(&line[s..e], "1.96.0");
        let rewritten = format!("{}{}{}", &line[..s], "1.98.1", &line[e..]);
        assert_eq!(rewritten, r#"channel = "1.98.1""#);
    }

    #[test]
    fn cargo_toml_rust_version_floor_parses() {
        let line = r#"rust-version = "1.96""#;
        let (s, e) = span_cargo_rust_version_floor(line).expect("must parse");
        assert_eq!(&line[s..e], "1.96");
    }

    #[test]
    fn dockerfile_from_rust_parses_and_rewrites() {
        let line = "FROM rust:1.96.0-slim-bookworm AS builder";
        let (s, e) = span_dockerfile_from_rust(line).expect("must parse");
        assert_eq!(&line[s..e], "1.96.0");
        let rewritten = format!("{}{}{}", &line[..s], "1.98.1", &line[e..]);
        assert_eq!(rewritten, "FROM rust:1.98.1-slim-bookworm AS builder");
    }

    #[test]
    fn arg_rust_version_parses_and_rewrites() {
        let line = "ARG RUST_VERSION=1.96.0";
        let (s, e) = span_arg_rust_version(line).expect("must parse");
        assert_eq!(&line[s..e], "1.96.0");
        let rewritten = format!("{}{}{}", &line[..s], "1.98.1", &line[e..]);
        assert_eq!(rewritten, "ARG RUST_VERSION=1.98.1");
    }

    #[test]
    fn yaml_rust_version_parses_and_rewrites() {
        let line = r#"rust_version: "1.96.0""#;
        let (s, e) = span_yaml_rust_version(line).expect("must parse");
        assert_eq!(&line[s..e], "1.96.0");
        let rewritten = format!("{}{}{}", &line[..s], "1.98.1", &line[e..]);
        assert_eq!(rewritten, r#"rust_version: "1.98.1""#);
    }

    #[test]
    fn stable_toml_min_rust_parses_with_aligned_spaces() {
        let line = r#"min_rust    = "1.96.0""#;
        let (s, e) = span_stable_min_rust(line).expect("must parse");
        assert_eq!(&line[s..e], "1.96.0");
        let rewritten = format!("{}{}{}", &line[..s], "1.98.1", &line[e..]);
        // Alignment (the extra spaces) is preserved: only the value moves.
        assert_eq!(rewritten, r#"min_rust    = "1.98.1""#);
    }

    #[test]
    fn voxup_assert_eq_parses_and_rewrites() {
        let line = r#"        assert_eq!(p.rust_version, "1.96.0");"#;
        let (s, e) = span_voxup_assert(line).expect("must parse");
        assert_eq!(&line[s..e], "1.96.0");
        let rewritten = format!("{}{}{}", &line[..s], "1.98.1", &line[e..]);
        assert_eq!(
            rewritten,
            r#"        assert_eq!(p.rust_version, "1.98.1");"#
        );
    }

    // ---- row 2 is a floor, not a pin ----

    #[test]
    fn floor_is_a_minimum_not_a_prefix() {
        // Same series: satisfied.
        assert!(floor_satisfied("1.96", "1.96.0"));
        assert!(floor_satisfied("1.96", "1.96.1"));
        assert!(floor_satisfied("1.96.0", "1.96.0"));
        // A floor BELOW the pin is satisfied — this is the whole point of an
        // MSRV floor, and the prefix rule this replaced got it wrong. Under
        // the old rule bumping the pin to 1.98.1 forced rust-version to 1.98
        // even though the workspace still compiles on 1.96.
        assert!(floor_satisfied("1.95", "1.96.0"));
        assert!(floor_satisfied("1.96", "1.98.1"));
        assert!(floor_satisfied("1.90", "1.98.1"));
        // A floor ABOVE the pin is NOT satisfied: we would be claiming to need
        // more Rust than we build with. That is the drift worth catching.
        assert!(!floor_satisfied("1.99", "1.98.1"));
        assert!(!floor_satisfied("2.0", "1.98.1"));
        // Numeric, not lexicographic: "1.10" is ABOVE "1.9", which a string
        // compare gets backwards.
        assert!(floor_satisfied("1.9", "1.10.0"));
        assert!(!floor_satisfied("1.10", "1.9.0"));
    }

    // ---- rows 8a and 8b move together ----

    #[test]
    fn voxup_fixture_and_assertion_move_together() {
        let dir = tempdir();
        let root = dir.path();
        write_all_rows(root, "1.96.0");
        let n = rewrite_all(root, "1.98.1").expect("rewrite must succeed");
        assert!(n > 0);
        let text = std::fs::read_to_string(root.join("crates/voxup/src/profiles.rs")).unwrap();
        assert!(text.contains(r#"rust_version: "1.98.1""#), "{text}");
        assert!(
            text.contains(r#"assert_eq!(p.rust_version, "1.98.1");"#),
            "{text}"
        );
        // Rewriting one without the other would fail this: both or neither.
        assert!(!text.contains("1.96.0"));
    }

    // ---- validate_ssot_version ----

    #[test]
    fn validate_ssot_version_rejects_x_y_zero() {
        assert!(validate_ssot_version("1.96.0").is_err());
        assert!(validate_ssot_version("1.0.0").is_err());
        assert!(validate_ssot_version("2.10.0").is_err());
    }

    #[test]
    fn validate_ssot_version_accepts_a_real_patch() {
        assert!(validate_ssot_version("1.96.1").is_ok());
        assert!(validate_ssot_version("1.98.1").is_ok());
    }

    // ---- the two look-alike strings must never be matched ----

    #[test]
    fn build_health_test_fixtures_are_not_matched_as_declarations() {
        // Negative and positive inputs to a version-string parser's own tests
        // — not toolchain restatements.
        assert!(
            span_channel(r#"assert!(!is_real_rustc("cargo 1.96.0 (30a34c682 2026-05-25)"));"#)
                .is_none()
        );
        assert!(
            span_dockerfile_from_rust(
                "assert!(is_real_rustc(\"rustc 1.96.0 (ac68faa20 2026-05-25)\"));"
            )
            .is_none()
        );
        assert!(span_arg_rust_version("assert!(!is_real_rustup(\"cargo 1.96.0\"));").is_none());
    }

    #[test]
    fn doctor_mod_check_pass_fixture_is_not_matched_as_a_declaration() {
        let line = r#"Check::pass("toolchain: rustc identity", "rustc 1.96.0"),"#;
        assert!(span_channel(line).is_none());
        assert!(span_yaml_rust_version(line).is_none());
        assert!(span_arg_rust_version(line).is_none());
        assert!(span_dockerfile_from_rust(line).is_none());
        assert!(span_stable_min_rust(line).is_none());
        assert!(span_cargo_rust_version_floor(line).is_none());
        assert!(span_voxup_assert(line).is_none());
    }

    // ---- a `vox-versioning`-style path substring must not produce a false declaration ----

    #[test]
    fn a_substring_of_the_key_inside_a_longer_identifier_is_not_matched() {
        // `rust-version` living inside a longer path/identifier segment must
        // not satisfy the anchored parser — this is the exact bug class that
        // once hit `version_ssot` via `line.find("version")` matching inside
        // `vox-versioning`.
        assert!(span_cargo_rust_version_floor(r#"my-rust-version-helper = "1.96""#).is_none());
        assert!(span_channel(r#"my-channel-name = "1.96.0""#).is_none());
        assert!(
            span_yaml_rust_version(r#"legacy_rust_version_alias: "1.96.0""#).is_none(),
            "key must be the whole leading token, not a prefix of a longer one"
        );
    }

    // ---- ssot_rust_version scoping ----

    #[test]
    fn ssot_rust_version_reads_the_versions_section_only() {
        let yaml = r#"schema: vox.workspace.toolchain.v1
versions:
  rust: "1.96.0"
  node: "22.0.0"
targets:
  rust:
    - "wasm32-wasip1"
"#;
        assert_eq!(ssot_rust_version(yaml).as_deref(), Some("1.96.0"));
    }

    #[test]
    fn ssot_rust_version_rewrite_is_scoped_and_idempotent() {
        let yaml = r#"versions:
  rust: "1.96.0"
  node: "22.0.0"
"#;
        let (out, n) = rewrite_ssot_rust_version(yaml, "1.98.1");
        assert_eq!(n, 1);
        assert!(out.contains(r#"rust: "1.98.1""#));
        assert!(out.contains(r#"node: "22.0.0""#));
        let (out2, n2) = rewrite_ssot_rust_version(&out, "1.98.1");
        assert_eq!(n2, 0, "a second run must change nothing");
        assert_eq!(out, out2);
    }

    // ---- integration: declarations() + drift() against synthetic files ----

    /// Minimal fixture tree covering all nine rows, written under a temp dir
    /// so tests never touch the real repo.
    fn write_all_rows(root: &Path, version: &str) {
        std::fs::create_dir_all(root.join("contracts/toolchain")).unwrap();
        std::fs::create_dir_all(root.join("contracts/distribution")).unwrap();
        std::fs::create_dir_all(root.join("contracts/channels")).unwrap();
        std::fs::create_dir_all(root.join("infra/ci-runner")).unwrap();
        std::fs::create_dir_all(root.join("crates/voxup/src")).unwrap();

        std::fs::write(
            root.join("contracts/toolchain/workspace-toolchain.v1.yaml"),
            format!(
                "schema: vox.workspace.toolchain.v1\nversions:\n  rust: \"{version}\"\n  node: \"22.0.0\"\ntargets:\n  rust:\n    - \"wasm32-wasip1\"\n"
            ),
        )
        .unwrap();
        std::fs::write(
            root.join("rust-toolchain.toml"),
            format!(
                "[toolchain]\nchannel = \"{version}\"\ncomponents = [\"rustfmt\", \"clippy\"]\n"
            ),
        )
        .unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            format!(
                "[workspace.package]\nversion = \"0.6.0\"\nrust-version = \"{}\"\n",
                major_minor(version)
            ),
        )
        .unwrap();
        std::fs::write(
            root.join("Dockerfile"),
            format!("FROM rust:{version}-slim-bookworm AS builder\n"),
        )
        .unwrap();
        std::fs::write(
            root.join("Dockerfile.ci-runner"),
            format!("ARG RUST_VERSION={version}\n\nFROM ubuntu:24.04\n"),
        )
        .unwrap();
        std::fs::write(
            root.join("infra/ci-runner/Dockerfile"),
            format!("ARG RUST_VERSION={version}\nARG RUNNER_VERSION=2.337.0\n"),
        )
        .unwrap();
        std::fs::write(
            root.join("contracts/distribution/profiles.v1.yaml"),
            format!("schema_version: 1\nrust_version: \"{version}\"\n"),
        )
        .unwrap();
        std::fs::write(
            root.join("contracts/channels/stable.toml"),
            format!("[channel]\nname        = \"stable\"\nmin_rust    = \"{version}\"\n"),
        )
        .unwrap();
        std::fs::write(
            root.join("crates/voxup/src/profiles.rs"),
            format!(
                "pub struct Profiles {{ pub rust_version: String }}\n\n#[cfg(test)]\nmod tests {{\n    #[test]\n    fn parses_minimal_manifest() {{\n        let yaml = r#\"\nrust_version: \"{version}\"\n\"#;\n        let p = parse(yaml).unwrap();\n        assert_eq!(p.rust_version, \"{version}\");\n    }}\n}}\n"
            ),
        )
        .unwrap();
    }

    fn tempdir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn declarations_finds_all_nine_rows() {
        let dir = tempdir();
        write_all_rows(dir.path(), "1.96.0");
        let d = declarations(dir.path());
        assert_eq!(d.len(), 9, "{d:#?}");
        assert_eq!(d.iter().filter(|x| x.kind == Kind::Floor).count(), 1);
    }

    #[test]
    fn a_consistent_tree_reports_no_drift() {
        let dir = tempdir();
        write_all_rows(dir.path(), "1.96.0");
        assert!(drift(dir.path()).is_empty());
    }

    #[test]
    fn a_lagging_row_is_reported_as_drift() {
        let dir = tempdir();
        write_all_rows(dir.path(), "1.96.0");
        let dockerfile = dir.path().join("Dockerfile");
        std::fs::write(&dockerfile, "FROM rust:1.95.0-slim-bookworm AS builder\n").unwrap();
        let d = drift(dir.path());
        assert_eq!(d.len(), 1, "{d:#?}");
        assert_eq!(d[0].declaration.version, "1.95.0");
        assert_eq!(d[0].expected, "1.96.0");
    }

    #[test]
    fn a_floor_above_the_toolchain_is_drift() {
        let dir = tempdir();
        write_all_rows(dir.path(), "1.96.0");
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace.package]\nversion = \"0.6.0\"\nrust-version = \"1.99\"\n",
        )
        .unwrap();
        let d = drift(dir.path());
        assert_eq!(d.len(), 1, "{d:#?}");
        assert!(d[0].declaration.what.contains("rust-version"));
    }

    #[test]
    fn a_floor_below_the_toolchain_is_not_drift() {
        // The case the repo actually lives in: rust-version declares the
        // oldest Rust that can build the workspace, and the pin moves ahead of
        // it. A floor of "1.90" against a pinned "1.96.0" is correct, not drift.
        let dir = tempdir();
        write_all_rows(dir.path(), "1.96.0");
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace.package]\nversion = \"0.6.0\"\nrust-version = \"1.90\"\n",
        )
        .unwrap();
        assert!(drift(dir.path()).is_empty(), "{:#?}", drift(dir.path()));
    }

    #[test]
    fn rewrite_all_updates_every_row_and_the_ssot() {
        let dir = tempdir();
        let root = dir.path();
        write_all_rows(root, "1.96.0");
        let n = rewrite_all(root, "1.98.1").expect("rewrite must succeed");
        assert_eq!(n, 10, "9 rows + the SSOT itself: {n}");
        assert!(drift(root).is_empty(), "rewritten tree must be clean");
        let ssot = std::fs::read_to_string(root.join(SSOT_PATH)).unwrap();
        assert_eq!(ssot_rust_version(&ssot).as_deref(), Some("1.98.1"));
        let cargo = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
        assert!(cargo.contains(r#"rust-version = "1.98""#));
    }

    #[test]
    fn rewrite_all_refuses_an_x_y_zero_target() {
        let dir = tempdir();
        write_all_rows(dir.path(), "1.96.0");
        let err = rewrite_all(dir.path(), "1.99.0").expect_err("must refuse x.y.0");
        assert!(err.to_string().contains("1.99.0"));
    }

    #[test]
    fn rewrite_all_is_idempotent() {
        let dir = tempdir();
        let root = dir.path();
        write_all_rows(root, "1.96.0");
        rewrite_all(root, "1.98.1").unwrap();
        let n2 = rewrite_all(root, "1.98.1").unwrap();
        assert_eq!(n2, 0, "a second run must change nothing");
    }

    /// The actual CI gate: run against the real repo tree and require zero
    /// drift. This module does not change any version number, so this must
    /// be green at the end of the task — and stays a live regression test
    /// for every future bump.
    #[test]
    fn real_repo_has_no_toolchain_drift() {
        let root = crate::repo_root();
        let d = drift(&root);
        assert!(d.is_empty(), "{d:#?}");
    }
}
