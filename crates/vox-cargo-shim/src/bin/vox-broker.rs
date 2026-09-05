//! Read-only viewer for the vox build broker's audit log.
//!
//! The broker's data (`<broker_home>/metrics.jsonl`, `<broker_home>/broker.log`)
//! is already global and already structured; the only documented way to read it
//! was `tail -f` plus a `grep`. This binary is that command instead. It is
//! **strictly read-only**: it must never create, delete, or modify any broker
//! state, including the broker home directory itself.
//!
//! Subcommands:
//! - `vox-broker stats`  — render `metrics.jsonl` via `vox_build_queue::metrics`.
//! - `vox-broker log [-n N]` — last N lines of `broker.log` (default 20).
//! - `vox-broker status` — effective cap, reservation, busy-slot sample, home path.
//!
//! Argument parsing is hand-rolled (no `clap`) to keep this daemonless shim
//! crate's dependency footprint minimal, per project policy.
//!
//! See `docs/src/contributors/build-broker-usage.md`.

use std::path::Path;

/// A parsed subcommand. Kept separate from arg parsing so the parser and the
/// renderers are each independently testable.
#[derive(Debug, PartialEq, Eq)]
enum Cmd {
    Stats,
    Log { n: usize },
    Status,
}

const USAGE: &str = "usage: vox-broker <stats|log [-n N]|status>";

/// Parse `argv` (already stripped of the program name). Pure — no env or I/O —
/// so every edge case (missing subcommand, bad `-n`, unknown subcommand) is
/// directly testable.
fn parse_args(args: &[String]) -> Result<Cmd, String> {
    match args.first().map(String::as_str) {
        None => Err(USAGE.to_string()),
        Some("stats") => Ok(Cmd::Stats),
        Some("status") => Ok(Cmd::Status),
        Some("log") => parse_log_args(&args[1..]),
        Some(other) => Err(format!("vox-broker: unknown subcommand '{other}'\n{USAGE}")),
    }
}

fn parse_log_args(rest: &[String]) -> Result<Cmd, String> {
    let mut n: usize = 20;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "-n" => {
                let raw = rest
                    .get(i + 1)
                    .ok_or_else(|| "vox-broker: log: -n requires a value".to_string())?;
                n = raw
                    .parse::<usize>()
                    .map_err(|_| format!("vox-broker: log: invalid -n value: '{raw}'"))?;
                i += 2;
            }
            other => {
                return Err(format!(
                    "vox-broker: log: unknown argument '{other}'\n{USAGE}"
                ));
            }
        }
    }
    Ok(Cmd::Log { n })
}

/// A message shown when the relevant broker file has never been written —
/// which is the common case on a fresh machine, not an edge case, per the
/// task brief. Never a row of zeros.
fn no_data_message(what: &str, path: &Path) -> String {
    format!(
        "{what}: no data yet at {} (the build broker has not run here)",
        path.display()
    )
}

/// Render `vox-broker stats`: the existing `metrics::summarize` / `Summary::render`,
/// with a clear message in place of a zeroed-out summary when there's no data.
fn render_stats(root: &Path) -> String {
    let path = root.join("metrics.jsonl");
    if !path.is_file() {
        return no_data_message("stats", &path);
    }
    match vox_build_queue::metrics::summarize(&path) {
        Ok(s) if s.count == 0 => no_data_message("stats", &path),
        Ok(s) => s.render(),
        Err(e) => format!("stats: error reading {}: {e}", path.display()),
    }
}

/// Render `vox-broker log -n N`: the last `n` lines of `broker.log`.
fn render_log(root: &Path, n: usize) -> String {
    let path = root.join("broker.log");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return no_data_message("log", &path),
    };
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return no_data_message("log", &path);
    }
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

/// Whether the broker has ever actually run against `root`: not just "does
/// the directory exist" (a caller may have created it, e.g. `mktemp -d`, for
/// an isolated `VOX_BROKER_HOME` without ever running a build through it) but
/// whether any of the broker's own state is present.
fn broker_has_run(root: &Path) -> bool {
    root.join("slots").is_dir()
        || root.join("metrics.jsonl").is_file()
        || root.join("broker.log").is_file()
}

/// Render `vox-broker status`: effective cap, reservation, a busy-slot sample,
/// and the broker home path. Never creates the home directory or any slot
/// file — `probe_busy_slots` is a read-only probe that releases every lock it
/// takes before returning.
fn render_status(root: &Path, cap: usize, reserved: usize, busy: usize) -> String {
    let mut out = format!("broker home: {}\n", root.display());
    if !broker_has_run(root) {
        out.push_str("  (never run on this machine yet -- no state on disk)\n");
    }
    out.push_str(&format!("effective cap: {cap}"));
    if reserved > 0 {
        out.push_str(&format!(
            " (base minus {reserved} reserved for a containerized build domain)"
        ));
    }
    out.push('\n');
    out.push_str(&format!(
        "busy slots: {busy}/{cap} (sampled -- another process may take or release a slot \
         between reading this and acting on it)\n"
    ));
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = match parse_args(&args) {
        Ok(cmd) => cmd,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    };

    let root = vox_build_queue::global::global_root();
    let out = match cmd {
        Cmd::Stats => render_stats(&root),
        Cmd::Log { n } => render_log(&root, n),
        Cmd::Status => {
            let cap = vox_build_queue::global::effective_max_concurrent();
            let reserved = vox_build_queue::global::reserved_slots();
            let busy = vox_build_queue::global::probe_busy_slots(&root, cap).unwrap_or(0);
            render_status(&root, cap, reserved, busy)
        }
    };
    println!("{out}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // --- parse_args ----------------------------------------------------

    #[test]
    fn parses_stats_and_status() {
        assert_eq!(parse_args(&s(&["stats"])), Ok(Cmd::Stats));
        assert_eq!(parse_args(&s(&["status"])), Ok(Cmd::Status));
    }

    #[test]
    fn parses_log_default_n() {
        assert_eq!(parse_args(&s(&["log"])), Ok(Cmd::Log { n: 20 }));
    }

    #[test]
    fn parses_log_with_n_flag() {
        assert_eq!(parse_args(&s(&["log", "-n", "5"])), Ok(Cmd::Log { n: 5 }));
    }

    #[test]
    fn rejects_bad_n_value() {
        let err = parse_args(&s(&["log", "-n", "not-a-number"])).unwrap_err();
        assert!(err.contains("invalid -n value"), "got: {err}");
    }

    #[test]
    fn rejects_n_flag_missing_value() {
        let err = parse_args(&s(&["log", "-n"])).unwrap_err();
        assert!(err.contains("-n requires a value"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_subcommand() {
        let err = parse_args(&s(&["frobnicate"])).unwrap_err();
        assert!(err.contains("unknown subcommand"), "got: {err}");
    }

    #[test]
    fn rejects_missing_subcommand() {
        let err = parse_args(&s(&[])).unwrap_err();
        assert!(err.contains("usage:"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_log_argument() {
        let err = parse_args(&s(&["log", "--bogus"])).unwrap_err();
        assert!(err.contains("unknown argument"), "got: {err}");
    }

    // --- render_stats / render_log: never-run case ----------------------

    #[test]
    fn stats_never_run_says_no_data_not_zeros() {
        let tmp = tempfile::tempdir().unwrap();
        let out = render_stats(tmp.path());
        assert!(out.contains("no data yet"), "got: {out}");
        assert!(
            !out.contains("builds=0"),
            "must not read as a zeroed summary: {out}"
        );
    }

    #[test]
    fn stats_with_data_renders_summary() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = vox_build_queue::metrics::MetricRecord {
            ts_ms: 0,
            worktree: "wt".into(),
            subcmd: "test".into(),
            queue_wait_ms: 0,
            ran_ms: 5,
            argv_hash: 1,
            env_hash: 2,
            would_coalesce: false,
        };
        vox_build_queue::metrics::append(&tmp.path().join("metrics.jsonl"), &rec).unwrap();
        let out = render_stats(tmp.path());
        assert!(out.contains("builds=1"), "got: {out}");
    }

    #[test]
    fn log_never_run_says_no_data() {
        let tmp = tempfile::tempdir().unwrap();
        let out = render_log(tmp.path(), 20);
        assert!(out.contains("no data yet"), "got: {out}");
    }

    #[test]
    fn log_returns_last_n_lines() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("broker.log"), "a\nb\nc\nd\n").unwrap();
        assert_eq!(render_log(tmp.path(), 2), "c\nd");
        assert_eq!(render_log(tmp.path(), 100), "a\nb\nc\nd");
    }

    // --- render_status ---------------------------------------------------

    #[test]
    fn status_never_run_says_so_and_still_reports_cap() {
        let tmp = tempfile::tempdir().unwrap();
        let never_run = tmp.path().join("not-yet-created");
        let out = render_status(&never_run, 4, 0, 0);
        assert!(out.contains("never run on this machine yet"), "got: {out}");
        assert!(out.contains("effective cap: 4"), "got: {out}");
    }

    #[test]
    fn status_says_never_run_even_when_home_dir_exists_but_is_empty() {
        // Regression guard: a caller conventionally sets VOX_BROKER_HOME to a
        // `mktemp -d` path for isolation, which creates the directory itself
        // even though the broker has never written anything into it. The
        // never-run message must key off the broker's own state files, not
        // mere directory existence.
        let tmp = tempfile::tempdir().unwrap();
        let out = render_status(tmp.path(), 4, 0, 0);
        assert!(out.contains("never run on this machine yet"), "got: {out}");
    }

    #[test]
    fn status_mentions_reservation_when_set() {
        let tmp = tempfile::tempdir().unwrap();
        let out = render_status(tmp.path(), 3, 2, 1);
        assert!(out.contains("2 reserved"), "got: {out}");
        assert!(out.contains("busy slots: 1/3"), "got: {out}");
    }
}
