//! Classify cargo fingerprint-log lines (from
//! `CARGO_LOG=cargo::core::compiler::fingerprint=info`) into rebuild causes.
//! Pure text -> classification; capture/reporting live in the vox-cli
//! `graphify why-rebuilt` command. The parser NEVER guesses: unmatched dirty
//! reasons classify as `Unknown` with the raw line preserved.
//!
//! Known limitation (also printed by the CLI): this observes CHECK units, so
//! link-time-only pain (relinking the vox binary) is invisible here.

use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CauseClass {
    FeatureDrift,
    EnvChange,
    BuildScriptRerun,
    DepRebuilt,
    ConfigChange,
    FileDirty,
    Unknown,
}

impl CauseClass {
    pub fn as_str(self) -> &'static str {
        match self {
            CauseClass::FeatureDrift => "feature_drift",
            CauseClass::EnvChange => "env_change",
            CauseClass::BuildScriptRerun => "build_script_rerun",
            CauseClass::DepRebuilt => "dep_rebuilt",
            CauseClass::ConfigChange => "config_change",
            CauseClass::FileDirty => "file_dirty",
            CauseClass::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RebuildCause {
    pub krate: String,
    pub class: CauseClass,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Summary {
    pub total: usize,
    /// cause class name -> line count, deterministic order.
    pub counts: BTreeMap<String, usize>,
    pub unknown_rate: f64,
}

/// Extract the package name from the tracing span field `package_id=…`.
///
/// Handles BOTH formats cargo has emitted:
/// - legacy: `package_id=vox-db v0.1.0 (path+file:///…)`
/// - modern PackageIdSpec (cargo >=1.77): `package_id=path+file:///…/vox-db#0.1.0`
///   or `package_id=path+file:///…#vox-db@0.1.0`
///
/// Cargo always normalizes these URIs to forward slashes, even on Windows, so
/// `rsplit('/')` is sufficient — but split on either separator as a defensive
/// fallback in case that normalization ever changes.
///
/// Falls back to the token after "fingerprint dirty|error for ".
fn extract_package(line: &str) -> String {
    if let Some(i) = line.find("package_id=") {
        let tok = line[i + "package_id=".len()..]
            .split_whitespace()
            .next()
            .unwrap_or("?");
        if let Some((base, frag)) = tok.split_once('#') {
            return match frag.split_once('@') {
                // "…#vox-db@0.1.0" -> explicit name.
                Some((name, _)) => name.to_string(),
                // "…/vox-db#0.1.0" -> dir name is the crate name.
                None => base.rsplit(['/', '\\']).next().unwrap_or("?").to_string(),
            };
        }
        return tok.to_string();
    }
    for marker in ["fingerprint dirty for ", "fingerprint error for "] {
        if let Some(i) = line.find(marker) {
            let rest = &line[i + marker.len()..];
            return rest.split(['/', ' ']).next().unwrap_or("?").to_string();
        }
    }
    "?".to_string()
}

/// Substring classification of a fingerprint log line. Ordering matters:
/// specific causes (features/env/build-script/dep/config) are checked before
/// the broad `file ... changed` pattern — e.g. "the file `build.rs` has
/// changed (rerun-if-changed)" matches both `rerun-if` and `file`+`changed`;
/// checking build-script first is intentional, since the reason cargo cares
/// is the rerun-if directive, not that build.rs is source-dirty like any
/// other file. Anything unmatched is Unknown.
fn classify(line: &str) -> CauseClass {
    let l = line.to_lowercase();
    if l.contains("features changed") || l.contains("declared features") {
        CauseClass::FeatureDrift
    } else if l.contains("env variable") || l.contains("environment variable") {
        CauseClass::EnvChange
    } else if l.contains("rerun-if") || l.contains("build-script") || l.contains("build script") {
        CauseClass::BuildScriptRerun
    } else if l.contains("was rebuilt")
        || l.contains("dependency info changed")
        || l.contains("unit dependency")
        || l.contains("number of dependencies")
        // Modern cargo (>=1.77) phrasing: a dependency's own fingerprint went
        // stale, i.e. it was rebuilt. Must be checked before the generic
        // FsStatusOutdated fallback below, which would otherwise misclassify
        // this as FileDirty.
        || l.contains("staledepfingerprint")
    {
        CauseClass::DepRebuilt
    } else if l.contains("rustflags")
        || l.contains("profile configuration")
        || l.contains("config settings")
        || l.contains("compile kind")
        || l.contains("metadata changed")
        || l.contains("target configuration")
    {
        CauseClass::ConfigChange
    } else if (l.contains("file") && (l.contains("changed") || l.contains("stale")))
        || l.contains("fsstatusoutdated")
    {
        CauseClass::FileDirty
    } else {
        CauseClass::Unknown
    }
}

/// One entry per fingerprint log line that reports dirtiness. A crate usually
/// emits a header line ("fingerprint dirty for X", Unknown) plus a
/// "    dirty: <reason>" detail line; both are kept — `per_crate` collapses.
pub fn parse_fingerprint_log(log: &str) -> Vec<RebuildCause> {
    let mut out = Vec::new();
    for line in log.lines() {
        let is_fp = line.contains("cargo::core::compiler::fingerprint");
        let relevant = is_fp
            && (line.contains("fingerprint dirty for")
                || line.contains("fingerprint error for")
                || line.contains("dirty:")
                || line.contains("err:"));
        if !relevant {
            continue;
        }
        out.push(RebuildCause {
            krate: extract_package(line),
            class: classify(line),
            raw: line.to_string(),
        });
    }
    out
}

/// Collapse to one class per crate: keep the first specific (non-Unknown)
/// cause seen for that crate, in input order, over any Unknown entries.
/// Limitation: a crate with two DISTINCT specific causes keeps the first seen;
/// full line counts remain visible in `summarize`.
pub fn per_crate(causes: &[RebuildCause]) -> BTreeMap<String, CauseClass> {
    let mut out: BTreeMap<String, CauseClass> = BTreeMap::new();
    for c in causes {
        match out.get(&c.krate) {
            Some(CauseClass::Unknown) | None => {
                out.insert(c.krate.clone(), c.class);
            }
            Some(_) => {}
        }
    }
    out
}

/// Fraction of CRATES (not raw lines) that never resolved to a specific
/// cause. This is the right denominator for a "did the classifier fail"
/// gate: every dirty target unavoidably emits one reason-less header line
/// ("fingerprint dirty for X"), so the per-LINE unknown rate is structurally
/// inflated even when every crate resolved correctly via its detail line.
/// `summarize`'s line-level `unknown_rate` stays useful for the printed
/// summary table; this is what a "should we trust this run" gate should use.
pub fn per_crate_unknown_rate(per: &BTreeMap<String, CauseClass>) -> f64 {
    if per.is_empty() {
        return 0.0;
    }
    let unresolved = per.values().filter(|c| **c == CauseClass::Unknown).count();
    unresolved as f64 / per.len() as f64
}

pub fn summarize(causes: &[RebuildCause]) -> Summary {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for c in causes {
        *counts.entry(c.class.as_str().to_string()).or_insert(0) += 1;
    }
    let unknown = counts.get("unknown").copied().unwrap_or(0);
    let total = causes.len();
    Summary {
        total,
        counts,
        unknown_rate: if total == 0 {
            0.0
        } else {
            unknown as f64 / total as f64
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIXED: &str = include_str!("../tests/fixtures/fingerprint_mixed.log");

    #[test]
    fn classifies_each_known_cause() {
        let causes = parse_fingerprint_log(MIXED);
        let class_of = |k: &str| {
            causes
                .iter()
                .find(|c| c.krate == k && c.class != CauseClass::Unknown)
                .map(|c| c.class)
                .or_else(|| causes.iter().find(|c| c.krate == k).map(|c| c.class))
                .unwrap()
        };
        assert_eq!(class_of("vox-db"), CauseClass::FeatureDrift);
        assert_eq!(class_of("vox-cli"), CauseClass::DepRebuilt);
        assert_eq!(class_of("vox-secrets"), CauseClass::EnvChange);
        assert_eq!(class_of("vox-gui"), CauseClass::BuildScriptRerun);
        assert_eq!(class_of("vox-term"), CauseClass::ConfigChange);
        assert_eq!(class_of("vox-ast"), CauseClass::FileDirty);
    }

    #[test]
    fn unknown_preserves_raw_line() {
        let causes = parse_fingerprint_log(MIXED);
        let weird = causes.iter().find(|c| c.krate == "vox-weird").unwrap();
        assert_eq!(weird.class, CauseClass::Unknown);
        assert!(weird.raw.contains("some future cargo reason"));
    }

    #[test]
    fn garbage_input_yields_nothing() {
        assert!(parse_fingerprint_log("hello\nworld\n").is_empty());
    }

    #[test]
    fn summary_counts_and_unknown_rate() {
        let causes = parse_fingerprint_log(MIXED);
        let s = summarize(&causes);
        assert_eq!(s.total, causes.len());
        // 2 genuine unknowns: vox-db's header line (no reason keyword) plus
        // vox-weird's never-seen-before reason. vox-cli intentionally has
        // only its detail line so it doesn't add header noise.
        assert_eq!(*s.counts.get("unknown").unwrap(), 2);
        assert!(s.unknown_rate > 0.0 && s.unknown_rate < 0.5);
    }

    #[test]
    fn per_crate_dedup_prefers_specific_over_unknown() {
        // vox-db emits a bare "fingerprint dirty for" header (unknown) AND a
        // "features changed" detail line; per_crate must keep FeatureDrift.
        let causes = parse_fingerprint_log(MIXED);
        let per = per_crate(&causes);
        assert_eq!(*per.get("vox-db").unwrap(), CauseClass::FeatureDrift);
    }

    #[test]
    fn per_crate_unknown_rate_ignores_structural_header_noise() {
        // 9 of the fixture's 10 crates resolve to a specific cause via their
        // detail line even though EVERY crate also emits an unavoidable
        // reason-less header line (raw per-line unknown_rate is far higher
        // than this because of those headers — see the "45% on a real
        // capture where every crate resolved" case this regressed against).
        // Only vox-weird is genuinely unresolved (its one line is a reason
        // cargo has never used). The per-crate rate must reflect that
        // 1-in-10, not the line-level noise.
        let causes = parse_fingerprint_log(MIXED);
        let per = per_crate(&causes);
        let unresolved: Vec<&String> = per
            .iter()
            .filter(|(_, c)| **c == CauseClass::Unknown)
            .map(|(k, _)| k)
            .collect();
        assert_eq!(unresolved, vec!["vox-weird"]);
        assert_eq!(per.len(), 10, "per={per:?}");
        assert!((per_crate_unknown_rate(&per) - 1.0 / 10.0).abs() < 1e-9);
    }

    #[test]
    fn per_crate_unknown_rate_counts_truly_unresolved_crates() {
        let per = std::collections::BTreeMap::from([
            ("a".to_string(), CauseClass::FeatureDrift),
            ("b".to_string(), CauseClass::Unknown),
            ("c".to_string(), CauseClass::Unknown),
            ("d".to_string(), CauseClass::DepRebuilt),
        ]);
        assert_eq!(per_crate_unknown_rate(&per), 0.5);
    }

    #[test]
    fn per_crate_unknown_rate_empty_is_zero() {
        assert_eq!(
            per_crate_unknown_rate(&std::collections::BTreeMap::new()),
            0.0
        );
    }

    #[test]
    fn stale_dep_fingerprint_classifies_as_dep_rebuilt() {
        // Real cargo 1.96 phrasing captured 2026-07-02 from a live
        // `cargo check --workspace`: "FsStatusOutdated(StaleDepFingerprint {
        // unit: UnitIndex(N) })" means a dependency's own fingerprint went
        // stale — the modern spelling of "the dependency X was rebuilt".
        let causes = parse_fingerprint_log(MIXED);
        let per = per_crate(&causes);
        assert_eq!(*per.get("vox-arch-check").unwrap(), CauseClass::DepRebuilt);
    }

    #[test]
    fn modern_package_id_spec_yields_crate_names_not_urls() {
        // cargo >=1.77 span format: package_id=path+file:///…#0.1.0 (dir name
        // is the crate) and …#name@ver (explicit name). Never a URL as krate.
        let causes = parse_fingerprint_log(MIXED);
        let per = per_crate(&causes);
        assert_eq!(*per.get("vox-journal").unwrap(), CauseClass::FeatureDrift);
        assert_eq!(*per.get("vox-config").unwrap(), CauseClass::EnvChange);
        assert!(causes.iter().all(|c| !c.krate.contains("file:///")));
    }
}
