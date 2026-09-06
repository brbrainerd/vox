//! Home-directory resolution that **refuses** to guess.
//!
//! `dirs::home_dir()` (and `vox-config` `paths.rs`) fall back to `"."` or
//! `getpwuid` when `HOME` is unset. An uninstall pointed at CWD or the real
//! passwd home is an unrecoverable loss, so every mutating voxup path must
//! go through [`require_home`].

use anyhow::{Result, bail};
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

/// Resolve the user home from `HOME` (Unix) or `HOME`/`USERPROFILE` (Windows).
///
/// Refuses to run if the value is unset or empty. Does **not** consult
/// `dirs::home_dir()` — that helper falls back to `getpwuid` on macOS when
/// `HOME` is empty, which would silently target the real home directory.
pub fn require_home() -> Result<PathBuf> {
    let raw = match std::env::var("HOME") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            #[cfg(windows)]
            {
                match std::env::var("USERPROFILE") {
                    Ok(v) if !v.is_empty() => v,
                    _ => bail!(
                        "HOME is unset or empty (and USERPROFILE is unset or empty); \
                         refusing to run so we cannot target the current directory"
                    ),
                }
            }
            #[cfg(not(windows))]
            {
                bail!(
                    "HOME is unset or empty; refusing to run so we cannot target \
                     the current directory or the passwd fallback"
                );
            }
        }
    };
    Ok(PathBuf::from(raw))
}

/// Fail immediately if `home` is the process's real `$HOME` or is not under
/// `temp_root`. Test-only guard: a botched fixture must not operate on the
/// operator's home directory.
#[cfg(test)]
pub fn assert_test_home_is_isolated(home: &Path, temp_root: &Path, outer_home: &Path) {
    let home_abs = canonicalize_best_effort(home);
    let outer_abs = canonicalize_best_effort(outer_home);
    let temp_abs = canonicalize_best_effort(temp_root);
    assert_ne!(
        home_abs, outer_abs,
        "uninstall test resolved home {:?} equals the outer process HOME {:?}; aborting",
        home_abs, outer_abs
    );
    assert!(
        home_abs.starts_with(&temp_abs),
        "uninstall test home {:?} is not under the temp dir {:?}; aborting",
        home_abs,
        temp_abs
    );
}

#[cfg(test)]
fn canonicalize_best_effort(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn require_home_reads_home_env() {
        let _g = ENV_LOCK.lock().unwrap();
        let prev = std::env::var_os("HOME");
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: serialised by ENV_LOCK; restored below.
        unsafe { std::env::set_var("HOME", tmp.path()) };
        let got = require_home().unwrap();
        match prev {
            Some(v) => unsafe { std::env::set_var("HOME", v) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        assert_eq!(got, tmp.path());
    }

    #[test]
    fn require_home_refuses_unset_or_empty() {
        let _g = ENV_LOCK.lock().unwrap();
        let prev_home = std::env::var_os("HOME");
        let prev_up = std::env::var_os("USERPROFILE");
        unsafe {
            std::env::remove_var("HOME");
            std::env::remove_var("USERPROFILE");
        }
        let err = require_home().unwrap_err();
        unsafe { std::env::set_var("HOME", "") };
        let err_empty = require_home().unwrap_err();
        match prev_home {
            Some(v) => unsafe { std::env::set_var("HOME", v) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        match prev_up {
            Some(v) => unsafe { std::env::set_var("USERPROFILE", v) },
            None => unsafe { std::env::remove_var("USERPROFILE") },
        }
        assert!(
            err.to_string().contains("HOME is unset or empty"),
            "got: {err}"
        );
        assert!(
            err_empty.to_string().contains("HOME is unset or empty"),
            "got: {err_empty}"
        );
    }

    #[test]
    fn isolation_guard_rejects_real_home() {
        let outer = PathBuf::from("/Users/someone");
        let tmp = tempfile::tempdir().unwrap();
        let result = std::panic::catch_unwind(|| {
            assert_test_home_is_isolated(&outer, tmp.path(), &outer);
        });
        assert!(result.is_err(), "guard must panic when home == outer HOME");
    }

    #[test]
    fn isolation_guard_accepts_temp_home() {
        let outer = PathBuf::from("/Users/someone");
        let tmp = tempfile::tempdir().unwrap();
        assert_test_home_is_isolated(tmp.path(), tmp.path(), &outer);
    }
}
