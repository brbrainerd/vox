//! `vox doctor` — build broker visibility check.
//!
//! The build broker (a `cargo`-named shim on `PATH` ahead of the rustup proxy,
//! backed by a machine-wide N-slot file semaphore) has never run on any
//! machine, because nothing told a developer whether it was actually active.
//! This module reports three things: which `cargo` wins on `PATH`, whether the
//! broker is installed at all, and whether its concurrency cap is sane for
//! this machine.
//!
//! Worktree-gated: silent outside a vox checkout. The remediation strings
//! below name `scripts/broker-install.sh`, a repo-relative path — legitimate
//! advice only when the reader has a checkout to run it from.

use std::path::{Path, PathBuf};

use super::super::common::Check;

const CHECK_NAME_PATH: &str = "Build broker: PATH precedence";
const CHECK_NAME_INSTALL: &str = "Build broker: install state";
const CHECK_NAME_CONCURRENCY: &str = "Build broker: concurrency cap";

const INSTALL_HINT: &str =
    "run scripts/broker-install.sh (dry-run by default, pass --apply to write)";

fn cargo_exe_name() -> &'static str {
    if cfg!(windows) { "cargo.exe" } else { "cargo" }
}

/// Whether `root` looks like a vox checkout. Mirrors the pattern
/// `tail::run` already uses (`Cargo.toml` + `crates/vox-cli` present), just
/// rooted at a resolved repo root instead of the current directory so it
/// still works when `vox doctor` runs from a subdirectory.
fn in_vox_checkout(root: &Path) -> bool {
    root.join("Cargo.toml").is_file() && root.join("crates/vox-cli").is_dir()
}

/// The first `PATH` entry containing a `cargo` executable, or `None`. Pure
/// over its input — callers pass the raw `PATH` value rather than this
/// reading the environment itself, so it's directly testable.
fn first_cargo_dir(path_value: &str) -> Option<PathBuf> {
    std::env::split_paths(path_value).find(|dir| dir.join(cargo_exe_name()).is_file())
}

/// Which `cargo` currently wins on `PATH`.
#[derive(Debug, PartialEq, Eq)]
enum PathPrecedence {
    /// The broker's shim directory.
    Broker(PathBuf),
    /// The rustup proxy's directory (`~/.cargo/bin`).
    RustupProxy(PathBuf),
    /// Some other directory shadowing both.
    Other(PathBuf),
    /// No `cargo` executable found anywhere on `PATH`.
    None,
}

fn classify_path_precedence(
    path_value: &str,
    broker_bin: &Path,
    cargo_bin: &Path,
) -> PathPrecedence {
    match first_cargo_dir(path_value) {
        Some(dir) if dir == broker_bin => PathPrecedence::Broker(dir),
        Some(dir) if dir == cargo_bin => PathPrecedence::RustupProxy(dir),
        Some(dir) => PathPrecedence::Other(dir),
        None => PathPrecedence::None,
    }
}

/// `${VOX_BROKER_HOME:-$HOME/.vox/build-broker}`, parameterized so the default
/// substitution is testable without touching the process environment.
///
/// Mirrors `vox_build_queue::global::global_root`'s env-var handling exactly:
/// any explicit value (even an empty string) wins verbatim.
fn broker_home_dir(explicit: Option<&str>, home: &Path) -> PathBuf {
    match explicit {
        Some(d) => PathBuf::from(d),
        None => home.join(".vox").join("build-broker"),
    }
}

fn broker_bin_dir(broker_home: &Path) -> PathBuf {
    broker_home.join("bin")
}

fn cargo_bin_dir(home: &Path) -> PathBuf {
    home.join(".cargo").join("bin")
}

fn broker_shim_installed(broker_home: &Path) -> bool {
    broker_home.join("bin").join(cargo_exe_name()).is_file()
}

/// Max concurrent cargo builds machine-wide.
///
// vox:defactored-from vox-build-queue 2026-09-04
/// Mirrors `vox_build_queue::global::max_concurrent_from` exactly (including
/// its edge cases: an unparseable or `0` override falls back to the default,
/// not to the literal) without taking a `vox-build-queue` crate edge.
fn max_concurrent_from(raw: Option<&str>, parallelism: usize) -> usize {
    if let Some(n) = raw
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n >= 1)
    {
        return n;
    }
    (parallelism / 3).clamp(2, 8)
}

/// Slots reserved for a build domain the filesystem semaphore cannot see (a
/// containerised CI runner on this host).
///
// vox:defactored-from vox-build-queue 2026-09-04
/// Mirrors `vox_build_queue::global::reserved_slots_from` exactly (unset,
/// unparseable, or negative -> 0 reserved) without taking a
/// `vox-build-queue` crate edge.
fn reserved_slots_from(raw: Option<&str>) -> usize {
    raw.and_then(|v| v.parse::<usize>().ok()).unwrap_or(0)
}

/// The effective cap after the reservation is subtracted, floored at 1.
///
// vox:defactored-from vox-build-queue 2026-09-04
/// Mirrors `vox_build_queue::global::effective_max_concurrent_from` exactly
/// (the reservation applies *after* an explicit `VOX_BROKER_MAX_CONCURRENT`
/// override) without taking a `vox-build-queue` crate edge.
fn effective_cap_from(
    max_raw: Option<&str>,
    reserved_raw: Option<&str>,
    parallelism: usize,
) -> usize {
    let base = max_concurrent_from(max_raw, parallelism);
    let reserved = reserved_slots_from(reserved_raw);
    base.saturating_sub(reserved).max(1)
}

fn path_check(precedence: &PathPrecedence, installed: bool, broker_bin: &Path) -> Check {
    let not_installed_suffix = format!("broker not installed — {INSTALL_HINT}");
    let installed_suffix = format!(
        "broker is installed at {} but not first on PATH — a running shell keeps its \
         launch-time PATH, open a new terminal for the shim to take effect",
        broker_bin.display()
    );

    match precedence {
        PathPrecedence::Broker(dir) => Check::pass(
            CHECK_NAME_PATH,
            format!(
                "active: {} (broker shim wins over the rustup proxy)",
                dir.display()
            ),
        ),
        PathPrecedence::RustupProxy(dir) => Check::fail(
            CHECK_NAME_PATH,
            format!(
                "active: {} (rustup proxy) — {}",
                dir.display(),
                if installed {
                    &installed_suffix
                } else {
                    &not_installed_suffix
                }
            ),
        ),
        PathPrecedence::Other(dir) => Check::fail(
            CHECK_NAME_PATH,
            format!(
                "active: {} (neither the broker nor ~/.cargo/bin) — {}",
                dir.display(),
                if installed {
                    &installed_suffix
                } else {
                    &not_installed_suffix
                }
            ),
        ),
        PathPrecedence::None => Check::fail(
            CHECK_NAME_PATH,
            format!(
                "no cargo executable found on PATH — {}",
                if installed {
                    &installed_suffix
                } else {
                    &not_installed_suffix
                }
            ),
        ),
    }
}

fn install_check(broker_home: &Path, installed: bool) -> Check {
    if !broker_home.is_dir() {
        return Check::fail(
            CHECK_NAME_INSTALL,
            format!("{} does not exist — {INSTALL_HINT}", broker_home.display()),
        );
    }
    if installed {
        Check::pass(
            CHECK_NAME_INSTALL,
            format!(
                "{} exists with bin/{} present",
                broker_home.display(),
                cargo_exe_name()
            ),
        )
    } else {
        Check::fail(
            CHECK_NAME_INSTALL,
            format!(
                "{} exists but bin/{} is missing — {INSTALL_HINT}",
                broker_home.display(),
                cargo_exe_name()
            ),
        )
    }
}

fn cap_check(max_raw: Option<&str>, reserved_raw: Option<&str>, cores: usize) -> Check {
    let cap = effective_cap_from(max_raw, reserved_raw, cores);
    let reserved = reserved_slots_from(reserved_raw);
    let reserved_suffix = if reserved > 0 {
        format!(" ({reserved} reserved for a containerized build domain)")
    } else {
        String::new()
    };
    if cap > cores {
        Check::fail(
            CHECK_NAME_CONCURRENCY,
            format!(
                "effective cap {cap}{reserved_suffix} exceeds {cores} logical core(s) detected \
                 on this machine — a cap that large caps nothing; unset \
                 VOX_BROKER_MAX_CONCURRENT or lower it"
            ),
        )
    } else {
        Check::pass(
            CHECK_NAME_CONCURRENCY,
            format!("effective cap {cap}{reserved_suffix} ({cores} logical core(s) detected)"),
        )
    }
}

pub fn run(checks: &mut Vec<Check>) {
    let root = crate::commands::ci::repo_root();
    if !in_vox_checkout(&root) {
        return;
    }

    let home = crate::fs_utils::user_home_dir();
    let broker_home = broker_home_dir(std::env::var("VOX_BROKER_HOME").ok().as_deref(), &home);
    let broker_bin = broker_bin_dir(&broker_home);
    let cargo_bin = cargo_bin_dir(&home);
    let installed = broker_shim_installed(&broker_home);

    let path_value = std::env::var("PATH").unwrap_or_default();
    let precedence = classify_path_precedence(&path_value, &broker_bin, &cargo_bin);
    checks.push(path_check(&precedence, installed, &broker_bin));
    checks.push(install_check(&broker_home, installed));

    let cap_raw = std::env::var("VOX_BROKER_MAX_CONCURRENT").ok();
    let reserved_raw = std::env::var("VOX_BROKER_RESERVED_SLOTS").ok();
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    checks.push(cap_check(
        cap_raw.as_deref(),
        reserved_raw.as_deref(),
        cores,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_exe(dir: &Path, name: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(name), "").unwrap();
    }

    fn join_paths(dirs: &[&Path]) -> String {
        std::env::join_paths(dirs.iter().copied())
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    // --- first_cargo_dir / classify_path_precedence -------------------------

    #[test]
    fn shim_before_cargo_bin_is_active() {
        let tmp = tempfile::tempdir().unwrap();
        let broker_bin = tmp.path().join("broker/bin");
        let cargo_bin = tmp.path().join("cargo/bin");
        write_exe(&broker_bin, cargo_exe_name());
        write_exe(&cargo_bin, cargo_exe_name());

        let path_value = join_paths(&[&broker_bin, &cargo_bin]);
        assert_eq!(
            classify_path_precedence(&path_value, &broker_bin, &cargo_bin),
            PathPrecedence::Broker(broker_bin.clone())
        );
        assert_eq!(first_cargo_dir(&path_value), Some(broker_bin));
    }

    #[test]
    fn cargo_bin_before_shim_is_inactive() {
        let tmp = tempfile::tempdir().unwrap();
        let broker_bin = tmp.path().join("broker/bin");
        let cargo_bin = tmp.path().join("cargo/bin");
        write_exe(&broker_bin, cargo_exe_name());
        write_exe(&cargo_bin, cargo_exe_name());

        let path_value = join_paths(&[&cargo_bin, &broker_bin]);
        assert_eq!(
            classify_path_precedence(&path_value, &broker_bin, &cargo_bin),
            PathPrecedence::RustupProxy(cargo_bin)
        );
    }

    #[test]
    fn neither_present_is_inactive() {
        let tmp = tempfile::tempdir().unwrap();
        let broker_bin = tmp.path().join("broker/bin");
        let cargo_bin = tmp.path().join("cargo/bin");
        let other = tmp.path().join("other/bin");
        std::fs::create_dir_all(&other).unwrap();

        let path_value = join_paths(&[&other]);
        assert_eq!(
            classify_path_precedence(&path_value, &broker_bin, &cargo_bin),
            PathPrecedence::None
        );
    }

    #[test]
    fn path_entry_without_cargo_is_skipped_not_matched() {
        let tmp = tempfile::tempdir().unwrap();
        let broker_bin = tmp.path().join("broker/bin");
        let cargo_bin = tmp.path().join("cargo/bin");
        let empty_dir = tmp.path().join("empty/bin");
        std::fs::create_dir_all(&empty_dir).unwrap();
        write_exe(&broker_bin, cargo_exe_name());

        // empty_dir exists but has no cargo executable in it -- it must be
        // skipped, not treated as a (non-)match that short-circuits the scan.
        let path_value = join_paths(&[&empty_dir, &broker_bin, &cargo_bin]);
        assert_eq!(
            classify_path_precedence(&path_value, &broker_bin, &cargo_bin),
            PathPrecedence::Broker(broker_bin)
        );
    }

    #[test]
    fn other_shadowing_dir_is_reported_distinctly() {
        let tmp = tempfile::tempdir().unwrap();
        let broker_bin = tmp.path().join("broker/bin");
        let cargo_bin = tmp.path().join("cargo/bin");
        let other = tmp.path().join("other/bin");
        write_exe(&other, cargo_exe_name());

        let path_value = join_paths(&[&other, &broker_bin, &cargo_bin]);
        assert_eq!(
            classify_path_precedence(&path_value, &broker_bin, &cargo_bin),
            PathPrecedence::Other(other)
        );
    }

    // --- broker_home_dir / broker_shim_installed ----------------------------

    #[test]
    fn broker_home_dir_defaults_under_home() {
        let home = Path::new("/home/dev");
        assert_eq!(
            broker_home_dir(None, home),
            home.join(".vox").join("build-broker")
        );
    }

    #[test]
    fn broker_home_dir_honors_explicit_override() {
        let home = Path::new("/home/dev");
        assert_eq!(
            broker_home_dir(Some("/custom/broker"), home),
            PathBuf::from("/custom/broker")
        );
    }

    #[test]
    fn broker_shim_installed_reflects_bin_cargo_presence() {
        let tmp = tempfile::tempdir().unwrap();
        let broker_home = tmp.path().join("broker-home");
        assert!(!broker_shim_installed(&broker_home));

        write_exe(&broker_home.join("bin"), cargo_exe_name());
        assert!(broker_shim_installed(&broker_home));
    }

    // --- in_vox_checkout -----------------------------------------------------

    #[test]
    fn recognizes_a_vox_checkout() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        std::fs::create_dir_all(tmp.path().join("crates/vox-cli")).unwrap();
        assert!(in_vox_checkout(tmp.path()));
    }

    #[test]
    fn rejects_a_non_checkout_directory() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!in_vox_checkout(tmp.path()));
    }

    // --- max_concurrent_from ---------------------------------------------------

    #[test]
    fn explicit_valid_override_wins() {
        assert_eq!(max_concurrent_from(Some("5"), 24), 5);
    }

    #[test]
    fn zero_empty_and_non_numeric_fall_back_to_default() {
        assert_eq!(max_concurrent_from(Some("0"), 24), 8);
        assert_eq!(max_concurrent_from(Some(""), 24), 8);
        assert_eq!(max_concurrent_from(Some("xx"), 24), 8);
        assert_eq!(max_concurrent_from(None, 24), 8);
    }

    #[test]
    fn clamp_holds_at_both_ends() {
        assert_eq!(max_concurrent_from(None, 3), 2); // 3/3=1, clamped up to 2
        assert_eq!(max_concurrent_from(None, 24), 8); // 24/3=8, clamped at 8
    }

    // --- reserved_slots_from / effective_cap_from ---------------------------

    #[test]
    fn reserved_slots_ignores_bad_values() {
        assert_eq!(reserved_slots_from(None), 0);
        assert_eq!(reserved_slots_from(Some("0")), 0);
        assert_eq!(reserved_slots_from(Some("-1")), 0);
        assert_eq!(reserved_slots_from(Some("nope")), 0);
        assert_eq!(reserved_slots_from(Some("3")), 3);
    }

    #[test]
    fn effective_cap_applies_reservation_after_override() {
        assert_eq!(effective_cap_from(None, None, 24), 8); // unchanged, no reservation
        assert_eq!(effective_cap_from(None, Some("3"), 24), 5); // base 8 - 3
        assert_eq!(effective_cap_from(None, Some("99"), 24), 1); // floors at 1
        assert_eq!(effective_cap_from(Some("4"), Some("1"), 24), 3); // override still reserved-down
        assert_eq!(effective_cap_from(Some("4"), Some("10"), 24), 1);
    }

    // --- cap_check ---------------------------------------------------------

    #[test]
    fn cap_greater_than_cores_is_flagged() {
        let check = cap_check(Some("16"), None, 8);
        assert!(!check.pass, "a cap exceeding core count must be flagged");
        assert!(check.detail.contains("exceeds"));
    }

    #[test]
    fn cap_within_cores_passes() {
        let check = cap_check(None, None, 24);
        assert!(check.pass);
    }

    #[test]
    fn cap_check_reports_reservation_when_set() {
        let check = cap_check(None, Some("3"), 24);
        assert!(check.pass);
        assert!(check.detail.contains("effective cap 5"));
        assert!(check.detail.contains("3 reserved"));
    }

    // --- path_check / install_check -----------------------------------------

    #[test]
    fn path_check_passes_when_broker_active() {
        let broker_bin = PathBuf::from("/home/dev/.vox/build-broker/bin");
        let check = path_check(
            &PathPrecedence::Broker(broker_bin.clone()),
            true,
            &broker_bin,
        );
        assert!(check.pass);
    }

    #[test]
    fn path_check_fails_and_names_new_terminal_when_installed_but_shadowed() {
        let broker_bin = PathBuf::from("/home/dev/.vox/build-broker/bin");
        let cargo_bin = PathBuf::from("/home/dev/.cargo/bin");
        let check = path_check(&PathPrecedence::RustupProxy(cargo_bin), true, &broker_bin);
        assert!(!check.pass);
        assert!(check.detail.contains("new terminal"));
        assert!(!check.detail.contains("broker-install.sh"));
    }

    #[test]
    fn path_check_fails_and_names_installer_when_not_installed() {
        let broker_bin = PathBuf::from("/home/dev/.vox/build-broker/bin");
        let cargo_bin = PathBuf::from("/home/dev/.cargo/bin");
        let check = path_check(&PathPrecedence::RustupProxy(cargo_bin), false, &broker_bin);
        assert!(!check.pass);
        assert!(check.detail.contains("scripts/broker-install.sh"));
    }

    #[test]
    fn install_check_reports_absent_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let broker_home = tmp.path().join("does-not-exist");
        let check = install_check(&broker_home, false);
        assert!(!check.pass);
        assert!(check.detail.contains("scripts/broker-install.sh"));
    }

    #[test]
    fn install_check_passes_when_shim_present() {
        let tmp = tempfile::tempdir().unwrap();
        let broker_home = tmp.path().join("broker-home");
        write_exe(&broker_home.join("bin"), cargo_exe_name());
        let check = install_check(&broker_home, true);
        assert!(check.pass);
    }
}
