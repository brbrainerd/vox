//! `vox doctor` — disk footprint check.
//!
//! Users cannot see what Vox costs them on disk until something fills up.
//! This module reports, per directory Vox writes to: its resolved path, its
//! measured size, and which environment variable relocates or bounds it — or
//! that none does. The "no override" case is the finding this check exists
//! to surface, not a footnote.
//!
//! Strictly read-only: this module never creates, moves, copies, or deletes
//! anything, and never even creates the directories it measures (several of
//! `vox-config`'s own resolvers `create_dir_all` as a side effect, so this
//! module deliberately does NOT call those directly for the platform data
//! dir — see [`data_dir_readonly`]). Sizing tolerates a missing directory
//! (reported as absent, not an error) and caps the walk so a pathologically
//! large or deep tree cannot make `vox doctor` hang — see
//! [`MAX_ENTRIES_PER_DIR`].

use std::path::{Path, PathBuf};

use super::super::common::Check;

/// Cap on filesystem entries walked per measured directory. Each entry costs
/// one `stat` (via `WalkDir`'s `metadata()`, never a file-content read), so
/// the worst case for one directory is `O(MAX_ENTRIES_PER_DIR)` stat calls —
/// tens of milliseconds even on a cold cache — and the walk stops as soon as
/// the cap is hit instead of continuing to enumerate a directory with
/// millions of entries (e.g. a misconfigured cache pointed at something huge).
/// When the cap is hit the reported size is a lower bound, flagged as such.
const MAX_ENTRIES_PER_DIR: usize = 50_000;

/// Directory size in bytes, tolerating missing/unreadable entries.
/// Returns `(bytes, truncated)`; `truncated` is `true` when the walk hit
/// [`MAX_ENTRIES_PER_DIR`] before finishing, so `bytes` is a lower bound.
fn dir_size_capped(path: &Path) -> (u64, bool) {
    let mut bytes = 0u64;
    let mut n = 0usize;
    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.path_is_symlink() {
            continue;
        }
        n += 1;
        if n > MAX_ENTRIES_PER_DIR {
            return (bytes, true);
        }
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                bytes = bytes.saturating_add(meta.len());
            }
        }
    }
    (bytes, false)
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut n = bytes as f64;
    let mut unit = 0usize;
    while n >= 1024.0 && unit < UNITS.len() - 1 {
        n /= 1024.0;
        unit += 1;
    }
    format!("{n:.1} {}", UNITS[unit])
}

/// Render a directory's measured size, tolerating absence.
fn size_summary(path: &Path) -> String {
    if !path.is_dir() {
        return "absent (0 B)".to_string();
    }
    let (bytes, truncated) = dir_size_capped(path);
    let size = human_bytes(bytes);
    if truncated {
        format!(">= {size} (walk capped at {MAX_ENTRIES_PER_DIR} entries)")
    } else {
        size
    }
}

/// Whether `root` looks like a vox checkout — gates the repo-scoped graphify
/// row, whose remediation names a repo env var only meaningful with a
/// checkout in scope. Small and duplicated rather than importing
/// `build_broker`'s private helper of the same shape (same crate, but that
/// module's `in_vox_checkout` is not `pub(crate)`).
fn in_vox_checkout(root: &Path) -> bool {
    root.join("Cargo.toml").is_file() && root.join("crates/vox-cli").is_dir()
}

/// `~/.vox/bin` — voxup's toolchain/binary install directory. Pure over its
/// `home` argument.
///
/// Deliberately does **not** honor `VOX_HOME`: `crates/voxup/src/install.rs`
/// builds this path from the raw platform home directory, not
/// `vox_config::paths::dot_vox_user_dir()`. It is one of the ~30 other
/// home-relative `.vox` joins the `dot_vox_user_dir` doc comment names as
/// unmigrated.
fn voxup_bin_dir(home: &Path) -> PathBuf {
    home.join(".vox").join("bin")
}

/// Pure, read-only re-derivation of `vox_config::paths::data_dir()`'s path
/// resolution, WITHOUT that function's `create_dir_all` side effect — a
/// doctor check must never create the directory it is only trying to measure.
///
/// Mirrors `data_dir()` exactly: `VOX_DATA_DIR` (non-blank) wins; otherwise
/// the platform default (`<Application Support|APPDATA|XDG_DATA_HOME>/vox`).
///
// vox:defactored-from vox-config 2026-09-04
fn data_dir_readonly(home: &Path, vox_data_dir_raw: Option<&str>) -> Option<PathBuf> {
    if let Some(dir) = vox_data_dir_raw.filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    platform_data_dir_readonly(home).map(|base| base.join(vox_config::paths::APP_DIR_NAME))
}

// vox:defactored-from vox-config 2026-09-04
fn platform_data_dir_readonly(home: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    return std::env::var("APPDATA").ok().map(PathBuf::from);

    #[cfg(target_os = "macos")]
    return Some(home.join("Library").join("Application Support"));

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME")
            && !xdg.is_empty()
        {
            return Some(PathBuf::from(xdg));
        }
        Some(home.join(".local").join("share"))
    }
}

/// Pure, read-only re-derivation of the *legacy* graphify cache base directory
/// (the parent of every `<corpus_id>` subdirectory), mirroring
/// `vox_config::graphify::resolve_graphify_cache_dir`'s base-dir resolution
/// without requiring a specific corpus id.
///
/// This always returns a path, never `None`: `VOX_GRAPHIFY_DISABLE` and
/// `VOX_GRAPHIFY_CACHE_DIR` are declared knobs in `vox_config::graphify`, but
/// neither is actually consulted by the real graphify cache writer
/// (`crates/vox-cli/src/commands/graphify/mod.rs`) — see
/// [`graphify_cache_check`]'s wired-status note. Reporting `None` here would
/// claim the disable knob works, which it does not yet.
fn graphify_cache_base(repo_root: &Path, cache_dir_raw: Option<&str>) -> PathBuf {
    match cache_dir_raw {
        Some(v) if !v.trim().is_empty() => PathBuf::from(v),
        _ => repo_root
            .join(vox_config::paths::REPO_CACHE_DIR)
            .join(vox_config::paths::REPO_GRAPHIFY_CACHE_SUBDIR),
    }
}

fn bin_check(home: &Path) -> Check {
    let path = voxup_bin_dir(home);
    Check::fail(
        "Disk footprint: ~/.vox/bin (voxup toolchain installs)",
        format!(
            "{} — {} — no environment variable relocates or bounds this directory; \
             voxup's installer resolves it from the platform home directory directly, \
             independent of VOX_HOME",
            path.display(),
            size_summary(&path)
        ),
    )
}

fn dot_vox_cache_check() -> Check {
    let path = vox_config::paths::dot_vox_user_dir().join("cache");
    Check::pass(
        "Disk footprint: ~/.vox/cache (script + model catalog cache)",
        format!(
            "{} — {} — relocated by VOX_HOME (see also the VOX_HOME relocation \
             coverage check below for what VOX_HOME does and does not move)",
            path.display(),
            size_summary(&path)
        ),
    )
}

fn platform_data_dir_check(home: &Path, vox_data_dir_raw: Option<&str>) -> Option<Check> {
    let path = data_dir_readonly(home, vox_data_dir_raw)?;
    let active = vox_data_dir_raw.is_some_and(|v| !v.is_empty());
    Some(Check::pass(
        "Disk footprint: platform data dir (model telemetry, db)",
        format!(
            "{} — {} — bounded by VOX_DATA_DIR{}",
            path.display(),
            size_summary(&path),
            if active {
                " (active)"
            } else {
                " (not set — using platform default)"
            }
        ),
    ))
}

/// Shared disclaimer: `VOX_GRAPHIFY_CACHE_DIR`/`VOX_GRAPHIFY_DISABLE` are
/// declared config-registry knobs (`vox_config::graphify`) but are not
/// consumed anywhere in the real graphify cache writer,
/// `crates/vox-cli/src/commands/graphify/mod.rs` — setting either has no
/// effect on either cache root below today. A future contributor wiring
/// these up should start at that file's `primary_cache_dir` (and whatever
/// resolves the legacy `.vox/cache/graphify/<corpus_id>` root it does not
/// yet share code with).
const GRAPHIFY_ENV_NOT_WIRED_NOTE: &str = "declared in vox-config but NOT wired to the real \
    writer (crates/vox-cli/src/commands/graphify/mod.rs) — setting it has no effect today";

fn graphify_cache_check(repo_root: &Path, cache_dir_raw: Option<&str>) -> Check {
    let path = graphify_cache_base(repo_root, cache_dir_raw);
    let requested = cache_dir_raw.is_some_and(|v| !v.trim().is_empty());
    Check::pass(
        "Disk footprint: .vox/cache/graphify (legacy graphify corpus cache)",
        format!(
            "{} — {} — VOX_GRAPHIFY_CACHE_DIR ({}){}; VOX_GRAPHIFY_DISABLE ({})",
            path.display(),
            size_summary(&path),
            GRAPHIFY_ENV_NOT_WIRED_NOTE,
            if requested {
                " (a value is set, but it changed nothing — the writer ignored it)"
            } else {
                ""
            },
            GRAPHIFY_ENV_NOT_WIRED_NOTE,
        ),
    )
}

/// `.vox/cache/vox-graph` — the newer cache root some graphify corpora (e.g.
/// `crate-map`, via `primary_cache_dir`) now write to directly, alongside the
/// legacy `.vox/cache/graphify` root — see
/// `docs/src/architecture/graphify-duplicate-corpus-bytes-findings-2026.md`.
/// Reported as its own line item so a corpus that has moved to this root
/// isn't silently excluded from the disk-footprint total.
fn vox_graph_cache_check(repo_root: &Path) -> Check {
    let path = repo_root.join(vox_config::paths::REPO_VOX_GRAPH_CACHE_DIR);
    Check::pass(
        "Disk footprint: .vox/cache/vox-graph (current graphify corpus cache)",
        format!(
            "{} — {} — no environment variable relocates or disables this directory today",
            path.display(),
            size_summary(&path)
        ),
    )
}

/// Explicit callout of `VOX_HOME`'s partial relocation, so setting it does
/// not silently leave some `.vox` consumers behind. See the doc comment on
/// `vox_config::paths::dot_vox_user_dir` for the authoritative list.
fn vox_home_partial_relocation_check(vox_home_raw: Option<&str>) -> Check {
    let active = vox_home_raw.is_some_and(|v| !v.trim().is_empty());
    if !active {
        return Check::pass(
            "Disk footprint: VOX_HOME relocation coverage",
            "VOX_HOME is not set — every .vox consumer uses the default $HOME/.vox, so there \
             is no partial relocation to report",
        );
    }
    Check::fail(
        "Disk footprint: VOX_HOME relocation coverage",
        "VOX_HOME is set: it relocates vox_config::paths::dot_vox_user_dir() (script cache, \
         the ~/.vox/cache row above) but NOT crates/vox-secrets's auth_json::vox_dir() — the \
         secrets vault fallback key (~/.vox/.vox-master-key) stays under $HOME/.vox regardless \
         — nor roughly thirty other home-relative .vox joins across voxup, vox-cli, vox-gui, \
         vox-cli-core, vox-runtime, and vox-plugin-host. This is a partial relocation by design, \
         not a full one; do not assume VOX_HOME moves everything under ~/.vox",
    )
}

pub fn run(checks: &mut Vec<Check>) {
    let home = crate::fs_utils::user_home_dir();

    checks.push(bin_check(&home));
    checks.push(dot_vox_cache_check());

    let vox_data_dir_raw = std::env::var("VOX_DATA_DIR").ok();
    if let Some(check) = platform_data_dir_check(&home, vox_data_dir_raw.as_deref()) {
        checks.push(check);
    }

    let root = crate::commands::ci::repo_root();
    if in_vox_checkout(&root) {
        let cache_dir_raw = std::env::var(vox_config::graphify::GRAPHIFY_CACHE_DIR_ENV).ok();
        checks.push(graphify_cache_check(&root, cache_dir_raw.as_deref()));
        checks.push(vox_graph_cache_check(&root));
    }

    let vox_home_raw = std::env::var(vox_config::paths::HOME_OVERRIDE_ENV_VAR).ok();
    checks.push(vox_home_partial_relocation_check(vox_home_raw.as_deref()));
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- dir_size_capped / human_bytes ---------------------------------

    #[test]
    fn dir_size_capped_sums_file_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.bin"), vec![0u8; 100]).unwrap();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub/b.bin"), vec![0u8; 50]).unwrap();
        let (bytes, truncated) = dir_size_capped(tmp.path());
        assert_eq!(bytes, 150);
        assert!(!truncated);
    }

    #[test]
    fn dir_size_capped_reports_absent_as_zero_via_size_summary() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist");
        assert_eq!(size_summary(&missing), "absent (0 B)");
    }

    #[test]
    fn human_bytes_picks_sensible_units() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(500), "500 B");
        assert_eq!(human_bytes(2048), "2.0 KB");
        assert_eq!(human_bytes(150 * 1024 * 1024), "150.0 MB");
    }

    // --- voxup_bin_dir / in_vox_checkout ---------------------------------

    #[test]
    fn voxup_bin_dir_ignores_vox_home_by_construction() {
        // No VOX_HOME parameter exists on this function at all — it only
        // takes `home`, which is the guarantee that it can never honor
        // VOX_HOME even by accident.
        let home = Path::new("/home/alice");
        assert_eq!(voxup_bin_dir(home), Path::new("/home/alice/.vox/bin"));
    }

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

    // --- data_dir_readonly -------------------------------------------------

    #[test]
    fn data_dir_readonly_honors_explicit_override() {
        let home = Path::new("/home/alice");
        assert_eq!(
            data_dir_readonly(home, Some("/mnt/vox-data")),
            Some(PathBuf::from("/mnt/vox-data"))
        );
    }

    #[test]
    fn data_dir_readonly_blank_falls_back_to_platform_default() {
        let home = Path::new("/home/alice");
        let got = data_dir_readonly(home, Some("")).unwrap();
        assert!(got.ends_with("vox"), "{got:?}");
    }

    #[test]
    fn data_dir_readonly_never_creates_anything() {
        // Regression guard for the constraint that this check is strictly
        // read-only: resolving the path for a home directory that does not
        // exist must not create it (unlike `vox_config::paths::data_dir()`,
        // which calls `create_dir_all`).
        let tmp = tempfile::tempdir().unwrap();
        let ghost_home = tmp.path().join("ghost-home");
        assert!(!ghost_home.exists());
        let _ = data_dir_readonly(&ghost_home, None);
        assert!(
            !ghost_home.exists(),
            "data_dir_readonly must never create the home dir or anything under it"
        );
    }

    // --- graphify_cache_base ------------------------------------------------

    #[test]
    fn graphify_cache_base_default_under_dot_vox_cache() {
        let repo = Path::new("/repo");
        let got = graphify_cache_base(repo, None);
        assert_eq!(got, Path::new("/repo/.vox/cache/graphify"));
    }

    #[test]
    fn graphify_cache_base_honors_cache_dir_override() {
        let repo = Path::new("/repo");
        let got = graphify_cache_base(repo, Some("/mnt/gcache"));
        assert_eq!(got, Path::new("/mnt/gcache"));
    }

    #[test]
    fn graphify_cache_base_never_creates_anything() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("ghost-repo");
        assert!(!repo.exists());
        let _ = graphify_cache_base(&repo, None);
        assert!(
            !repo.exists(),
            "graphify_cache_base must never create the repo root or anything under it"
        );
    }

    // --- check builders: pass/fail wiring -----------------------------------

    #[test]
    fn bin_check_always_fails_no_override_exists() {
        let home = Path::new("/home/alice");
        let check = bin_check(home);
        assert!(!check.pass, "no env var relocates ~/.vox/bin — must fail");
        assert!(check.detail.contains("no environment variable"));
    }

    #[test]
    fn dot_vox_cache_check_passes_mechanism_exists() {
        let check = dot_vox_cache_check();
        assert!(check.pass);
        assert!(check.detail.contains("VOX_HOME"));
    }

    #[test]
    fn platform_data_dir_check_reports_active_override() {
        let home = Path::new("/home/alice");
        let check = platform_data_dir_check(home, Some("/mnt/vox-data")).unwrap();
        assert!(check.pass);
        assert!(check.detail.contains("(active)"));
        assert!(check.detail.contains("/mnt/vox-data"));
    }

    #[test]
    fn graphify_cache_check_flags_env_vars_as_not_wired() {
        let repo = Path::new("/repo");
        let check = graphify_cache_check(repo, None);
        assert!(check.pass);
        assert!(check.detail.contains("NOT wired"));
        assert!(
            check
                .detail
                .contains("crates/vox-cli/src/commands/graphify/mod.rs")
        );
    }

    #[test]
    fn graphify_cache_check_notes_ignored_override() {
        let repo = Path::new("/repo");
        let check = graphify_cache_check(repo, Some("/mnt/gcache"));
        assert!(check.pass);
        assert!(check.detail.contains("changed nothing"));
    }

    #[test]
    fn vox_graph_cache_check_reports_no_relocation_knob() {
        let repo = Path::new("/repo");
        let check = vox_graph_cache_check(repo);
        assert!(check.pass);
        assert!(check.detail.contains(".vox/cache/vox-graph"));
        assert!(check.detail.contains("no environment variable"));
    }

    #[test]
    fn vox_home_partial_relocation_check_flags_when_active() {
        let check = vox_home_partial_relocation_check(Some("/mnt/vox-home"));
        assert!(
            !check.pass,
            "an active VOX_HOME must surface the partial-relocation warning"
        );
        assert!(check.detail.contains(".vox-master-key"));
        assert!(check.detail.contains("vox-secrets"));
    }

    #[test]
    fn vox_home_partial_relocation_check_quiet_when_unset() {
        let check = vox_home_partial_relocation_check(None);
        assert!(check.pass);
    }
}
