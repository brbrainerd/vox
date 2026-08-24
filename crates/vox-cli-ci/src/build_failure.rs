//! Classifies a failed build's output, and returns the evidence for the verdict.
//!
//! Two failure classes on this host are not code defects, and both have been
//! misdiagnosed as such: a compiler that exits nonzero emitting no diagnostic
//! (memory pressure — the host idles at 1.6 GB free with a 1,193 MB peak rustc
//! working set), and stale or truncated artifacts left behind by an earlier
//! disk-full run, whose errors surface in the *next* build.
//!
//! The rule that matters: **`Real` outranks `Contention`**, and "real" is
//! decided by whether the output contains source diagnostics in *any* cargo
//! message format. A previous version tested `line.starts_with("error[")`,
//! which is false for `--message-format short` — the format this repo uses —
//! and so excused every genuine compile error as contention.

/// What a failed build's output actually indicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFailureKind {
    /// No diagnostics at all: the compiler died without saying why. Retry on a
    /// quiet host; check free RAM. Do not edit code.
    Contention,
    /// Stale or truncated artifacts, usually from an earlier disk-full run.
    /// Check free disk, then `cargo clean -p` the named crates.
    Corruption,
    /// A genuine compile error. Behave normally.
    Real,
}

/// A verdict plus the diagnostics that produced it.
///
/// The evidence is the point: a caller prints "classified as contention: 0
/// source diagnostics found" and a human overrules a wrong verdict at a glance.
/// A bare enum makes a misclassification invisible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFailure {
    pub kind: BuildFailureKind,
    pub diagnostics: Vec<String>,
}

/// Markers of stale or truncated artifacts. Only honored on diagnostic lines.
const CORRUPTION_MARKERS: &[&str] = &[
    "E0460",
    "only metadata stub found for",
    "found invalid metadata files for crate",
];

/// ENOSPC. Honored on any line that is itself an error line.
const DISK_FULL_MARKERS: &[&str] = &["os error 112", "not enough space on the disk"];

/// Cargo's own summary lines. They name no source location and carry no error
/// code, so they are not evidence of a code defect.
const CARGO_SUMMARIES: &[&str] = &[
    "could not compile",
    "build failed",
    "aborting due to",
    "failed to run custom build command",
    "linking with",
    "process didn't exit successfully",
];

/// Classify the output of a failed build.
///
/// `truncated` says whether the capture was cut short. A truncated capture with
/// no diagnostics classifies `Real`, because it is indistinguishable from a
/// build whose diagnostics were simply not captured.
pub fn classify_build_failure(output: &str, truncated: bool) -> BuildFailure {
    let diagnostics = source_diagnostics(output);

    if truncated && diagnostics.is_empty() {
        return BuildFailure {
            kind: BuildFailureKind::Real,
            diagnostics,
        };
    }

    let disk_full = output
        .lines()
        .any(|l| is_error_line(l) && DISK_FULL_MARKERS.iter().any(|m| l.contains(m)));
    let stale_artifacts = diagnostics
        .iter()
        .any(|d| CORRUPTION_MARKERS.iter().any(|m| d.contains(m)));
    if disk_full || stale_artifacts {
        return BuildFailure {
            kind: BuildFailureKind::Corruption,
            diagnostics,
        };
    }

    // Real before contention: one genuine error anywhere outranks contention
    // anywhere. Contention is the fallback and needs no marker list.
    let kind = if diagnostics.is_empty() {
        BuildFailureKind::Contention
    } else {
        BuildFailureKind::Real
    };
    BuildFailure { kind, diagnostics }
}

/// Every source diagnostic in the output, in any cargo message format.
fn source_diagnostics(output: &str) -> Vec<String> {
    let lines: Vec<&str> = output.lines().collect();
    let mut found = Vec::new();

    for (i, raw) in lines.iter().enumerate() {
        let line = raw.trim_end();
        let t = line.trim_start();
        if t.is_empty() {
            continue;
        }

        // `--message-format json`.
        let json = t.starts_with('{') && t.contains("\"level\":\"error\"");
        // Any format, coded: `error[E0610]`, with or without a path prefix.
        let coded = t.contains("error[E");
        // `--message-format short`: `path:line:col: error…`.
        let short = is_short_format_diagnostic(t);
        // Full format, uncoded: `error: msg` with a `-->` location beneath it.
        let full_uncoded = t.starts_with("error: ")
            && !CARGO_SUMMARIES
                .iter()
                .any(|s| t["error: ".len()..].starts_with(s))
            && lines[i + 1..]
                .iter()
                .find(|n| !n.trim().is_empty())
                .is_some_and(|n| n.trim_start().starts_with("--> "));

        if json || coded || short || full_uncoded {
            found.push(line.to_string());
        }
    }
    found
}

/// `path:line:col: error…` — the short format's shape.
fn is_short_format_diagnostic(t: &str) -> bool {
    let Some(idx) = t.find(": error") else {
        return false;
    };
    // The prefix must end in `:<digits>:<digits>`; that is what separates a
    // diagnostic from prose that happens to contain the word "error".
    let mut parts = t[..idx].rsplit(':');
    let col = parts.next().unwrap_or_default();
    let line = parts.next().unwrap_or_default();
    let numeric = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    numeric(col) && numeric(line)
}

/// True for a line that is itself an error report, as opposed to prose or test
/// output that merely quotes one. Anchoring on this is what stops a test *named*
/// after an error code from tripping the corruption markers.
fn is_error_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("error") || is_short_format_diagnostic(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(output: &str) -> BuildFailureKind {
        classify_build_failure(output, false).kind
    }

    /// The regression that motivated this rewrite. This repo builds with
    /// `--message-format short`, where the path comes first and the previous
    /// `starts_with("error[")` predicate returned false — classifying every
    /// real compile error as `Contention`, i.e. "retry, don't edit code".
    #[test]
    fn short_format_diagnostics_are_real() {
        let out = "crates\\vox-gui\\src\\commands\\mic.rs:169:51: error[E0610]: \
                   `{integer}` is a primitive type and therefore doesn't have fields\n\
                   error: could not compile `vox-gui` (lib) due to 1 previous error";
        let f = classify_build_failure(out, false);
        assert_eq!(f.kind, BuildFailureKind::Real);
        assert_eq!(f.diagnostics.len(), 1, "{:?}", f.diagnostics);
    }

    #[test]
    fn full_format_diagnostics_are_real() {
        // Coded, and uncoded-with-location-arrow: both must be found.
        let coded = "error[E0308]: mismatched types\n --> src/lib.rs:3:9";
        let uncoded = "error: expected one of `,` or `}`, found `;`\n --> src/lib.rs:7:1";
        assert_eq!(kind(coded), BuildFailureKind::Real);
        assert_eq!(kind(uncoded), BuildFailureKind::Real);
    }

    #[test]
    fn json_format_diagnostics_are_real() {
        let out = r#"{"reason":"compiler-message","message":{"level":"error","rendered":"boom"}}"#;
        assert_eq!(kind(out), BuildFailureKind::Real);
    }

    /// Real beats contention. A mixed run — crate A a genuine error, crate B a
    /// linker failure — previously returned `Contention` because contention was
    /// evaluated first.
    #[test]
    fn a_real_error_outranks_a_linker_failure() {
        let out = "src/a.rs:1:1: error[E0425]: cannot find value `nope` in this scope\n\
                   error: linking with `lld-link` failed: exit code: 1\n\
                   error: could not compile `b` (lib)";
        assert_eq!(kind(out), BuildFailureKind::Real);
    }

    /// Zero diagnostics on a failed build is the only contention signal we
    /// need. `cc failed` / `Permission denied` / `failed to run custom build
    /// command` are deliberately NOT markers — each also names a real bug, and
    /// a diagnostic-free build script failure lands here anyway.
    #[test]
    fn no_diagnostics_is_contention_and_says_so() {
        let out = "error: could not compile `glob` (lib)\n\
                   error: failed to run custom build command for `zstd-sys v2.0.16`";
        let f = classify_build_failure(out, false);
        assert_eq!(f.kind, BuildFailureKind::Contention);
        assert!(
            f.diagnostics.is_empty(),
            "evidence must show zero: {:?}",
            f.diagnostics
        );
    }

    /// Corruption markers must be anchored to diagnostic lines. A test *named*
    /// after E0460 previously classified the whole run as `Corruption`.
    #[test]
    fn corruption_markers_are_anchored_to_diagnostics() {
        let real = "test metadata::e0460_stale_rlib_is_corruption ... ok\n\
                    src/a.rs:2:2: error[E0308]: mismatched types";
        assert_eq!(kind(real), BuildFailureKind::Real);

        let corrupt =
            "src/lib.rs:1:1: error[E0460]: found possibly newer version of crate `windows`";
        assert_eq!(kind(corrupt), BuildFailureKind::Corruption);

        let stub =
            "error: only metadata stub found for `rlib` dependency `std`\n --> src/lib.rs:1:1";
        assert_eq!(kind(stub), BuildFailureKind::Corruption);
    }

    /// ENOSPC is the one proven cause in the corpus, and it poisons the target
    /// directory for the *next* build. It must be reachable on a cargo error
    /// line, not only on a source diagnostic.
    #[test]
    fn enospc_is_corruption() {
        let out = "error: couldn't create a temp dir: There is not enough space \
                   on the disk. (os error 112)";
        assert_eq!(kind(out), BuildFailureKind::Corruption);
        // ...but the same string inside test output is not.
        assert_eq!(
            kind("test disk::reports_os_error_112 ... ok"),
            BuildFailureKind::Contention,
        );
    }

    /// A truncated capture is indistinguishable from a no-diagnostic build.
    /// `Real` is the safe direction: it costs an investigation, where a wrong
    /// `Contention` costs an edit to working code.
    #[test]
    fn truncated_capture_without_diagnostics_is_real() {
        let out = "error: could not compile `glob` (lib)";
        assert_eq!(
            classify_build_failure(out, true).kind,
            BuildFailureKind::Real
        );
        assert_eq!(
            classify_build_failure(out, false).kind,
            BuildFailureKind::Contention
        );
    }

    #[test]
    fn empty_output_is_contention_with_no_evidence() {
        let f = classify_build_failure("", false);
        assert_eq!(f.kind, BuildFailureKind::Contention);
        assert!(f.diagnostics.is_empty());
    }
}
