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

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use regex::Regex;

use super::constants::REPO_SLUG;
use super::runner_scale::{gh_json, quiet_command};

/// One parsed `oom-kill:constraint=CONSTRAINT_MEMCG` kernel log line — the
/// single `dmesg` line that carries both the killed process name and the
/// container's full cgroup id together (`oom_memcg=/docker/<id>,...,task=<name>`).
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
    fn parses_multiple_events_and_skips_noise_between_them() {
        let text = format!(
            "[1.0] noise\n\
             [2.0] oom-kill:constraint=CONSTRAINT_MEMCG,oom_memcg=/docker/{a},task_memcg=/docker/{a},task=cargo,pid=1,uid=0\n\
             [3.0] Memory cgroup out of memory: Killed process 1 (cargo)\n\
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
}
