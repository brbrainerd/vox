//! Host-side detection of self-hosted CI runner containers hard-killed by
//! their own memory cgroup limit — and reporting that evidence directly on
//! the PR/run that was affected.
//!
//! Two things this exists to work around (see the design doc for the
//! evidence): a runner container cannot read `dmesg` itself (no `CAP_SYSLOG`
//! by default, correctly so), and a job that gets OOM-killed cannot run its
//! own `if: always()` report step (the runner agent process dies with the
//! container — there is no "after" for that same job). So detection and
//! reporting both live here, on the host-side autoscaler tick
//! (`vox ci runner-scale`, invoked every 2 minutes), not inside the job.
//!
//! Design: docs/superpowers/specs/2026-07-07-ci-runner-memory-budget-and-oom-visibility-design.md
//!
//! **Currently implemented in this file:** dmesg-line parsing only
//! (`parse_oom_events`, below) — a pure string-parsing function with no
//! `gh`/`dmesg`/reporting IO. Dedup persistence, container-name resolution,
//! GitHub job correlation, and PR comment composition/posting all land in
//! follow-up tasks of the implementation plan; until then nothing in this
//! module actually reports anything anywhere.

use std::path::PathBuf;

use regex::Regex;

/// One parsed `oom-kill:constraint=CONSTRAINT_MEMCG` kernel log line — the
/// single `dmesg` line that carries both the killed process name and the
/// container's full cgroup id together (`oom_memcg=/docker/<id>,...,task=<name>`).
///
/// `#[allow(dead_code)]`: not yet constructed outside `#[cfg(test)]` — the
/// dedup-persistence task later in the implementation plan is the first
/// caller. Remove this allow once that task lands and wires it up.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OomEvent {
    /// Raw matched line, used as the dedup key (content-based, not
    /// timestamp-based — avoids parsing dmesg's locale-dependent date format).
    pub raw_line: String,
    /// Killed process name, e.g. "rustdoc".
    pub process: String,
    /// Full 64-char docker container/cgroup id.
    pub cgroup_id: String,
}

/// `#[allow(dead_code)]`: only called from `parse_oom_events` today, which
/// is itself only called from tests until the orchestration task
/// (`scan_and_report_oom_events`) lands later in the implementation plan.
#[allow(dead_code)]
fn oom_line_regex() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"oom_memcg=/docker/([0-9a-f]{64}).*?task=([^,]+)")
            .expect("static oom-kill regex must compile")
    })
}

/// Parse `dmesg` output for `oom-kill:constraint=CONSTRAINT_MEMCG` lines,
/// extracting the killed process name and the container's cgroup id from
/// each. Lines that don't match (the overwhelming majority of `dmesg`) are
/// skipped. Pure — no IO.
///
/// `#[allow(dead_code)]`: only called from tests today — the orchestration
/// task (`scan_and_report_oom_events`) later in the implementation plan is
/// the first non-test caller. Remove this allow once that task lands.
#[allow(dead_code)]
pub fn parse_oom_events(dmesg_text: &str) -> Vec<OomEvent> {
    let re = oom_line_regex();
    dmesg_text
        .lines()
        .filter(|l| l.contains("oom-kill:constraint=CONSTRAINT_MEMCG"))
        .filter_map(|line| {
            let caps = re.captures(line)?;
            Some(OomEvent {
                raw_line: line.to_string(),
                cgroup_id: caps.get(1)?.as_str().to_string(),
                process: caps.get(2)?.as_str().to_string(),
            })
        })
        .collect()
}

// --- dedup persistence across ticks -----------------------------------

fn oom_seen_path() -> PathBuf {
    crate::fs_utils::user_home_dir()
        .join(".vox")
        .join("ci-runner-oom-seen.json")
}

fn read_oom_seen() -> Vec<String> {
    std::fs::read_to_string(oom_seen_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_oom_seen(seen: &[String]) {
    let p = oom_seen_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(seen) {
        let _ = std::fs::write(p, s);
    }
}

/// Events from `events` whose raw line isn't already in `seen`. Pure.
pub fn new_events<'a>(events: &'a [OomEvent], seen: &[String]) -> Vec<&'a OomEvent> {
    events
        .iter()
        .filter(|e| !seen.iter().any(|s| s == &e.raw_line))
        .collect()
}

/// Cap on the seen-list so its state file never grows unbounded.
const OOM_SEEN_MAX: usize = 500;

/// Append newly-seen raw lines to the existing seen-list, capped to
/// [`OOM_SEEN_MAX`] most-recent entries (oldest dropped first). Pure.
pub fn append_seen(mut seen: Vec<String>, newly_seen: &[String]) -> Vec<String> {
    seen.extend(newly_seen.iter().cloned());
    if seen.len() > OOM_SEEN_MAX {
        let drop = seen.len() - OOM_SEEN_MAX;
        seen.drain(0..drop);
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_captured_oom_line() {
        // Verbatim (container id truncated for readability, but kept a valid
        // 64-char hex string) shape of a real line captured via
        // `wsl -e dmesg -T` during the 2026-07-07 investigation.
        let line = "[Tue Jul  7 07:53:04 2026] oom-kill:constraint=CONSTRAINT_MEMCG,\
                     nodemask=(null),cpuset=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
                     mems_allowed=0,oom_memcg=/docker/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
                     task_memcg=/docker/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
                     task=rustdoc,pid=15612,uid=0";
        let events = parse_oom_events(line);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].process, "rustdoc");
        assert_eq!(
            events[0].cgroup_id,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(events[0].raw_line, line);
    }

    #[test]
    fn ignores_unrelated_dmesg_lines() {
        let text = "[1.0] some unrelated boot message\n\
                     [2.0] another line entirely\n";
        assert!(parse_oom_events(text).is_empty());
    }

    #[test]
    fn parses_multiple_events_in_order_and_skips_a_malformed_decoy() {
        // The decoy line on [3.0] deliberately contains the
        // "oom-kill:constraint=CONSTRAINT_MEMCG" substring the initial
        // `.filter()` checks for, but its cgroup id is truncated (not a
        // valid 64-char hex string), so it fails `re.captures` and must be
        // dropped by the `filter_map`'s `?` short-circuit -- proving the
        // noise-skipping is enforced by the regex capture, not merely by
        // lines never containing the substring to begin with.
        let text = format!(
            "[1.0] noise\n\
             [2.0] oom-kill:constraint=CONSTRAINT_MEMCG,oom_memcg=/docker/{a},task_memcg=/docker/{a},task=cargo,pid=1,uid=0\n\
             [3.0] oom-kill:constraint=CONSTRAINT_MEMCG,oom_memcg=/docker/deadbeef,task_memcg=/docker/deadbeef,task=truncated,pid=9,uid=0\n\
             [4.0] more noise\n\
             [5.0] oom-kill:constraint=CONSTRAINT_MEMCG,oom_memcg=/docker/{b},task_memcg=/docker/{b},task=rustc,pid=2,uid=0\n",
            a = "a".repeat(64),
            b = "b".repeat(64),
        );
        let events = parse_oom_events(&text);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].process, "cargo");
        assert_eq!(events[1].process, "rustc");
    }

    #[test]
    fn new_events_filters_out_already_seen_lines() {
        let events = vec![
            OomEvent {
                raw_line: "line-a".to_string(),
                process: "cargo".to_string(),
                cgroup_id: "a".repeat(64),
            },
            OomEvent {
                raw_line: "line-b".to_string(),
                process: "rustc".to_string(),
                cgroup_id: "b".repeat(64),
            },
        ];
        let seen = vec!["line-a".to_string()];
        let fresh = new_events(&events, &seen);
        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0].raw_line, "line-b");
    }

    #[test]
    fn new_events_returns_all_when_seen_is_empty() {
        let events = vec![OomEvent {
            raw_line: "line-a".to_string(),
            process: "cargo".to_string(),
            cgroup_id: "a".repeat(64),
        }];
        assert_eq!(new_events(&events, &[]).len(), 1);
    }

    #[test]
    fn append_seen_caps_to_max_dropping_oldest_first() {
        let seen: Vec<String> = (0..OOM_SEEN_MAX).map(|i| format!("old-{i}")).collect();
        let newly = vec!["new-1".to_string(), "new-2".to_string()];
        let updated = append_seen(seen, &newly);
        assert_eq!(updated.len(), OOM_SEEN_MAX);
        // The two oldest entries were dropped to make room.
        assert!(!updated.contains(&"old-0".to_string()));
        assert!(!updated.contains(&"old-1".to_string()));
        assert!(updated.contains(&"old-2".to_string()));
        // The new entries are present.
        assert!(updated.contains(&"new-1".to_string()));
        assert!(updated.contains(&"new-2".to_string()));
    }

    #[test]
    fn append_seen_under_cap_keeps_everything() {
        let seen = vec!["a".to_string()];
        let updated = append_seen(seen, &["b".to_string()]);
        assert_eq!(updated, vec!["a".to_string(), "b".to_string()]);
    }
}
