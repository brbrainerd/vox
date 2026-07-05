//! T1.2 RED test: a lightweight, maintainable guard against Tier-B events
//! (high-frequency, broadcast-only — `TokenStreamed`, heartbeats, throughput/cost
//! ticks, lock/diag chatter) ever being wired into the durable op-log.
//!
//! `OperationKind` (`vox-orchestrator-queue`) is `#[non_exhaustive]` and has no
//! structural relationship to `AgentEventKind`, so there is no compiler-checked
//! way to assert "no durable-write call site passes a Tier-B-shaped kind" —
//! the two enums live in different crates with different variant sets. Instead
//! this is a source-grep guard: it greps every `record_operation` /
//! `OpLog::record` / `record_persisted` call site across `vox-orchestrator` and
//! `vox-orchestrator-mcp` and asserts none of them mention `TokenStreamed` (or
//! any of the other known Tier-B kind names) as an `OperationKind` payload.
//!
//! This complements (does not replace) `events::is_tier_a`/`is_tier_b`, which is
//! the compile-time-checked exhaustive classification of `AgentEventKind` itself.

use std::path::Path;

/// Kind names that must never appear as an `OperationKind::` variant argument
/// at a durable-write call site. Consumes `events::TIER_B_KIND_NAMES` directly
/// (the FULL, authoritative Tier-B set derived from `is_tier_a`'s exhaustive
/// match) rather than a second, hand-maintained short list — so this guard
/// and the compile-checked classification cannot silently drift apart.
const FORBIDDEN_DURABLE_KIND_NAMES: &[&str] = vox_orchestrator::events::TIER_B_KIND_NAMES;

/// Recursively collect `.rs` file contents under `dir`.
fn collect_rs_sources(dir: &Path, out: &mut Vec<(std::path::PathBuf, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip build output / vendored dirs if ever encountered.
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n == "target")
            {
                continue;
            }
            collect_rs_sources(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
            && let Ok(content) = std::fs::read_to_string(&path)
        {
            out.push((path, content));
        }
    }
}

/// For each line that invokes a durable-write function
/// (`record_operation`/`OpLog::record`/`record_persisted`), scan a small
/// trailing window of subsequent lines (the call's argument list, which is
/// multi-line in this codebase's formatting) for a forbidden Tier-B kind name
/// used as an `OperationKind::<Name>` constructor.
fn scan_for_forbidden_durable_kind(content: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = content.lines().collect();
    let mut hits = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let is_durable_call = line.contains("record_operation")
            || line.contains("OpLog::record")
            || line.contains(".record(")
            || line.contains("record_persisted");
        if !is_durable_call {
            continue;
        }
        // Look at this line plus the next ~15 lines for the OperationKind arg.
        let window_end = (i + 15).min(lines.len());
        for w in &lines[i..window_end] {
            for name in FORBIDDEN_DURABLE_KIND_NAMES {
                let needle = format!("OperationKind::{name}");
                if w.contains(&needle) {
                    hits.push((i + 1, w.trim().to_string()));
                }
            }
            // A closing call-terminator on its own line ends this call's argument list.
            if w.trim() == ");" {
                break;
            }
        }
    }
    hits
}

#[test]
fn no_durable_write_site_records_a_tier_b_kind() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("workspace root");

    let scan_dirs = [
        workspace_root.join("crates/vox-orchestrator/src"),
        workspace_root.join("crates/vox-orchestrator-mcp/src"),
    ];

    let mut sources = Vec::new();
    for dir in &scan_dirs {
        assert!(
            dir.is_dir(),
            "expected scan dir to exist: {}",
            dir.display()
        );
        collect_rs_sources(dir, &mut sources);
    }
    assert!(
        sources.len() > 10,
        "sanity check: expected to find more than 10 .rs files under {scan_dirs:?}, found {}",
        sources.len()
    );

    let mut violations = Vec::new();
    for (path, content) in &sources {
        for (line_no, line) in scan_for_forbidden_durable_kind(content) {
            violations.push(format!("{}:{line_no}: {line}", path.display()));
        }
    }

    assert!(
        violations.is_empty(),
        "Tier-B event kind(s) found wired into a durable op-log write — this defeats \
         the T1.2 tier contract (Tier B must be broadcast-only, never durably \
         journaled). Violations:\n{}",
        violations.join("\n")
    );
}
