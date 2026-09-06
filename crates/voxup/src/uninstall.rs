//! `voxup uninstall` — allowlisted removal of installer-owned paths only.
//!
//! Allowlist (and nothing else):
//!   `~/.vox/bin`, `~/.vox/toolchains`, `~/.vox/run`
//!
//! Never `~/.vox` itself. Never `remove_dir_all` on a path outside the list.
//! Never delete `.vox-master-key`.

use anyhow::{Context, Result, bail};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::home::require_home;

/// Marker line written by [`crate::shell`] / `add_to_path`.
pub const VOXUP_MARKER: &str = "# Added by voxup";

/// Previous toolchains kept beside the active one when pruning on install.
pub const DEFAULT_KEEP_PREVIOUS: usize = 2;

/// Directories under `~/.vox` that uninstall may delete. Relative to `~/.vox`.
pub const ALLOWLIST: &[&str] = &["bin", "toolchains", "run"];

#[derive(Debug, Clone)]
pub struct UninstallOpts {
    pub dry_run: bool,
    /// Reserved for callers that prune-then-uninstall; tests pass it explicitly.
    #[allow(dead_code)]
    pub keep_previous: usize,
}

impl UninstallOpts {
    /// `--dry-run` is the default when stdin is not a TTY.
    pub fn from_cli(dry_run_flag: bool, apply_flag: bool) -> Self {
        let dry_run = if apply_flag {
            false
        } else if dry_run_flag {
            true
        } else {
            !io::stdin().is_terminal()
        };
        Self {
            dry_run,
            keep_previous: DEFAULT_KEEP_PREVIOUS,
        }
    }
}

#[derive(Debug, Default)]
pub struct UninstallReport {
    pub dry_run: bool,
    pub removed: Vec<PathBuf>,
    pub skipped_absent: Vec<PathBuf>,
    pub profile_edits: Vec<ProfileEdit>,
    pub cargo_vox: CargoVoxAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CargoVoxAction {
    #[default]
    NotPresent,
    LeftInPlace {
        reason: String,
    },
    WouldRemove(PathBuf),
    Removed(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileEdit {
    Removed {
        path: PathBuf,
        backup: PathBuf,
    },
    WouldEdit {
        path: PathBuf,
        before: String,
        after: String,
    },
    Manual {
        path: PathBuf,
        line: usize,
    },
    Unchanged {
        path: PathBuf,
    },
}

/// CLI entry: resolve HOME (refuse if unset) then uninstall.
pub fn run_uninstall(opts: UninstallOpts) -> Result<UninstallReport> {
    let home = require_home()?;
    uninstall_at(&home, &opts)
}

/// Uninstall against an explicit `home`. Callers that construct `home`
/// themselves (tests) must run [`crate::home::assert_test_home_is_isolated`]
/// first.
pub fn uninstall_at(home: &Path, opts: &UninstallOpts) -> Result<UninstallReport> {
    if home.as_os_str().is_empty() {
        bail!("home path is empty; refusing to run");
    }
    let vox = home.join(".vox");
    let mut report = UninstallReport {
        dry_run: opts.dry_run,
        ..UninstallReport::default()
    };

    // Provenance for `~/.cargo/bin/vox` requires both paths to still exist
    // (same inode, nlink > 1). Check it before deleting `~/.vox/bin`.
    report.cargo_vox = maybe_remove_cargo_hardlink(home, &vox.join("bin"), opts.dry_run)?;
    report.profile_edits = edit_profiles(home, &vox.join("bin"), opts.dry_run)?;

    for name in ALLOWLIST {
        let path = vox.join(name);
        if !path.exists() {
            report.skipped_absent.push(path);
            continue;
        }
        if opts.dry_run {
            println!("dry-run: would remove {}", path.display());
            report.removed.push(path);
            continue;
        }
        // Allowlisted names only — never `~/.vox` itself.
        remove_allowlisted(&path)?;
        println!("removed {}", path.display());
        report.removed.push(path);
    }

    print_report(&report)?;
    Ok(report)
}

fn remove_allowlisted(path: &Path) -> Result<()> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !ALLOWLIST.contains(&name) {
        bail!(
            "refusing to delete {}: not on the uninstall allowlist {:?}",
            path.display(),
            ALLOWLIST
        );
    }
    if path.is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("remove allowlisted dir {}", path.display()))?;
    } else {
        fs::remove_file(path)
            .with_context(|| format!("remove allowlisted file {}", path.display()))?;
    }
    Ok(())
}

/// Prune old `vox-<version>` toolchain dirs. Fail closed (delete nothing) if
/// `toolchains/active` is missing or unparseable.
pub fn prune_old_toolchains(toolchains_dir: &Path, keep_previous: usize) -> Result<Vec<PathBuf>> {
    let active_path = toolchains_dir.join("active");
    let active_raw = match fs::read_to_string(&active_path) {
        Ok(s) => s,
        Err(_) => {
            bail!(
                "toolchain prune: {} is missing; refusing to delete any toolchain",
                active_path.display()
            );
        }
    };
    let active = parse_active_version(&active_raw).ok_or_else(|| {
        anyhow::anyhow!(
            "toolchain prune: {} is unparseable ({:?}); refusing to delete any toolchain",
            active_path.display(),
            active_raw
        )
    })?;

    let mut versions: Vec<(semver::Version, PathBuf)> = Vec::new();
    let entries = match fs::read_dir(toolchains_dir) {
        Ok(e) => e,
        Err(_) => return Ok(Vec::new()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(ver_str) = name.strip_prefix("vox-") else {
            continue;
        };
        // Skip staging / retired siblings.
        if ver_str.ends_with(".incoming") || ver_str.ends_with(".retired") {
            continue;
        }
        if let Ok(ver) = semver::Version::parse(ver_str) {
            versions.push((ver, path));
        }
    }
    versions.sort_by(|a, b| b.0.cmp(&a.0));

    let keep_active = versions
        .iter()
        .find(|(v, _)| v.to_string() == active || format!("{v}") == active);
    if keep_active.is_none() {
        // Active points at a version that is not on disk. Fail closed rather
        // than guessing which dirs are stale.
        bail!(
            "toolchain prune: active version {active} has no matching vox-{active} dir; \
             refusing to delete any toolchain"
        );
    }

    let mut kept = 0usize;
    let mut removed = Vec::new();
    for (ver, path) in &versions {
        if ver.to_string() == active {
            continue;
        }
        if kept < keep_previous {
            kept += 1;
            continue;
        }
        fs::remove_dir_all(path)
            .with_context(|| format!("prune old toolchain {}", path.display()))?;
        removed.push(path.clone());
    }
    Ok(removed)
}

/// `Some` only when the file is a single non-empty version token with no
/// path separators.
pub fn parse_active_version(raw: &str) -> Option<String> {
    let v = raw.trim();
    if v.is_empty() {
        return None;
    }
    if v.contains('/') || v.contains('\\') || v.contains("..") {
        return None;
    }
    // Accept a semver, or a semver-ish token the install wrote (tag without v).
    if semver::Version::parse(v).is_ok() {
        return Some(v.to_string());
    }
    // Fail closed on anything we cannot parse as semver.
    None
}

/// Switch `toolchains/active` to the next-newest remaining version and return
/// that version. Fail closed if `active` is missing or unparseable.
pub fn rollback_active(toolchains_dir: &Path) -> Result<String> {
    let active_path = toolchains_dir.join("active");
    let raw = fs::read_to_string(&active_path).with_context(|| {
        format!(
            "rollback: {} is missing; nothing to roll back to",
            active_path.display()
        )
    })?;
    let current = parse_active_version(&raw).ok_or_else(|| {
        anyhow::anyhow!(
            "rollback: {} is unparseable ({:?}); refusing to change anything",
            active_path.display(),
            raw
        )
    })?;
    let mut versions: Vec<semver::Version> = Vec::new();
    if let Ok(entries) = fs::read_dir(toolchains_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(ver_str) = name.strip_prefix("vox-") else {
                continue;
            };
            if ver_str.ends_with(".incoming") || ver_str.ends_with(".retired") {
                continue;
            }
            if let Ok(ver) = semver::Version::parse(ver_str) {
                versions.push(ver);
            }
        }
    }
    versions.sort();
    versions.dedup();
    let current_ver = semver::Version::parse(&current)
        .with_context(|| format!("rollback: active {current} is not semver"))?;
    let previous = versions
        .iter()
        .rev()
        .find(|v| *v < &current_ver)
        .ok_or_else(|| {
            anyhow::anyhow!("rollback: no older toolchain than {current} is installed")
        })?;
    let next = previous.to_string();
    atomic_write(&active_path, &format!("{next}\n"))?;
    Ok(next)
}

pub fn run_rollback() -> Result<String> {
    let home = require_home()?;
    let tc = home.join(".vox").join("toolchains");
    rollback_active(&tc)
}

fn edit_profiles(home: &Path, bin_dir: &Path, dry_run: bool) -> Result<Vec<ProfileEdit>> {
    let mut edits = Vec::new();
    let mut candidates = vec![
        home.join(".bashrc"),
        home.join(".bash_profile"),
        home.join(".zshrc"),
        home.join(".profile"),
        home.join(".config").join("fish").join("config.fish"),
    ];
    if cfg!(windows) {
        let docs = home.join("Documents");
        candidates.push(docs.join("PowerShell/Microsoft.PowerShell_profile.ps1"));
        candidates.push(docs.join("WindowsPowerShell/Microsoft.PowerShell_profile.ps1"));
    }
    for path in candidates {
        if !path.exists() {
            continue;
        }
        edits.push(edit_one_profile(&path, bin_dir, dry_run)?);
    }
    Ok(edits)
}

fn edit_one_profile(path: &Path, bin_dir: &Path, dry_run: bool) -> Result<ProfileEdit> {
    let before =
        fs::read_to_string(path).with_context(|| format!("read profile {}", path.display()))?;
    match strip_voxup_block(&before) {
        Some(after) => {
            if dry_run {
                print_profile_diff(path, &before, &after);
                return Ok(ProfileEdit::WouldEdit {
                    path: path.to_path_buf(),
                    before,
                    after,
                });
            }
            let backup = backup_path(path)?;
            fs::copy(path, &backup)
                .with_context(|| format!("backup {} -> {}", path.display(), backup.display()))?;
            println!("profile backup: {}", backup.display());
            atomic_write(path, &after)?;
            Ok(ProfileEdit::Removed {
                path: path.to_path_buf(),
                backup,
            })
        }
        None => {
            if let Some(line) = line_containing_bin(&before, bin_dir) {
                eprintln!(
                    "voxup: {} line {line} mentions {} but has no `# Added by voxup` marker.\n\
                     Not editing. Remove that PATH entry by hand.",
                    path.display(),
                    bin_dir.display()
                );
                return Ok(ProfileEdit::Manual {
                    path: path.to_path_buf(),
                    line,
                });
            }
            Ok(ProfileEdit::Unchanged {
                path: path.to_path_buf(),
            })
        }
    }
}

/// Remove every exact contiguous two-line block (`# Added by voxup` + the
/// following PATH line). Returns `None` when the marker is absent.
pub fn strip_voxup_block(content: &str) -> Option<String> {
    if !content.lines().any(|l| l.trim_end() == VOXUP_MARKER) {
        return None;
    }
    let mut out = String::with_capacity(content.len());
    let mut lines = content.split_inclusive('\n').peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == VOXUP_MARKER {
            // Drop the marker and its one PATH line. Nothing else.
            let _ = lines.next();
            continue;
        }
        out.push_str(line);
    }
    Some(out)
}

fn line_containing_bin(content: &str, bin_dir: &Path) -> Option<usize> {
    let posix = bin_dir.to_string_lossy().replace('\\', "/");
    let native = bin_dir.to_string_lossy();
    content.lines().enumerate().find_map(|(i, line)| {
        if line.contains(posix.as_str()) || line.contains(native.as_ref()) {
            Some(i + 1)
        } else {
            None
        }
    })
}

fn backup_path(profile: &Path) -> Result<PathBuf> {
    let name = profile
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("profile");
    let ts = utc_iso8601()?;
    Ok(profile.with_file_name(format!("{name}.voxup-backup-{ts}")))
}

fn utc_iso8601() -> Result<String> {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before Unix epoch")?
        .as_secs();
    Ok(civil_iso8601(secs))
}

/// UTC `YYYY-MM-DDTHH:MM:SSZ` from a Unix timestamp. No extra crate.
fn civil_iso8601(secs: u64) -> String {
    let zdays = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let sec = rem % 60;
    // Howard Hinnant civil-from-days (public domain).
    let z = zdays + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{hour:02}:{min:02}:{sec:02}Z")
}

fn atomic_write(path: &Path, contents: &str) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(
        ".{}.voxup-tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("profile")
    ));
    {
        let mut f =
            fs::File::create(&tmp).with_context(|| format!("create temp {}", tmp.display()))?;
        f.write_all(contents.as_bytes())
            .with_context(|| format!("write temp {}", tmp.display()))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}

fn print_profile_diff(path: &Path, before: &str, after: &str) {
    println!("--- {}", path.display());
    println!("+++ {}", path.display());
    for line in before.lines() {
        if !after.lines().any(|a| a == line) {
            println!("- {line}");
        }
    }
    for line in after.lines() {
        if !before.lines().any(|b| b == line) {
            println!("+ {line}");
        }
    }
}

fn maybe_remove_cargo_hardlink(
    home: &Path,
    bin_dir: &Path,
    dry_run: bool,
) -> Result<CargoVoxAction> {
    let exe = if cfg!(windows) { "vox.exe" } else { "vox" };
    let cargo = home.join(".cargo").join("bin").join(exe);
    let canonical = bin_dir.join(exe);
    if !cargo.exists() {
        return Ok(CargoVoxAction::NotPresent);
    }
    if !is_proven_hardlink(&canonical, &cargo) {
        return Ok(CargoVoxAction::LeftInPlace {
            reason: format!(
                "{} is not a proven hardlink of {} (same inode, nlink > 1); leaving it",
                cargo.display(),
                canonical.display()
            ),
        });
    }
    if dry_run {
        println!("dry-run: would remove cargo hardlink {}", cargo.display());
        return Ok(CargoVoxAction::WouldRemove(cargo));
    }
    fs::remove_file(&cargo).with_context(|| format!("remove {}", cargo.display()))?;
    println!("removed cargo hardlink {}", cargo.display());
    Ok(CargoVoxAction::Removed(cargo))
}

/// Provenance: same inode (Unix) / same file index (Windows) and `nlink > 1`.
/// Never glob `~/.cargo/bin/vox*`.
pub fn is_proven_hardlink(a: &Path, b: &Path) -> bool {
    let Ok(ma) = fs::metadata(a) else {
        return false;
    };
    let Ok(mb) = fs::metadata(b) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ma.dev() == mb.dev() && ma.ino() == mb.ino() && ma.nlink() > 1
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        match (ma.file_index(), mb.file_index()) {
            (Some(ia), Some(ib)) => ia == ib && ma.number_of_links().unwrap_or(0) > 1,
            _ => false,
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (ma, mb);
        false
    }
}

fn print_report(report: &UninstallReport) -> Result<()> {
    if report.dry_run {
        println!("dry-run: no files were modified.");
    }
    if let CargoVoxAction::LeftInPlace { reason } = &report.cargo_vox {
        println!("voxup: {reason}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::assert_test_home_is_isolated;

    fn outer_home() -> PathBuf {
        PathBuf::from(std::env::var("HOME").expect("test host must have HOME"))
    }

    fn isolated_home() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().to_path_buf();
        assert_test_home_is_isolated(&home, tmp.path(), &outer_home());
        (tmp, home)
    }

    fn seed_user_files(vox: &Path) {
        fs::create_dir_all(vox).unwrap();
        fs::write(vox.join(".vox-master-key"), [0xABu8; 32]).unwrap();
        fs::write(vox.join("login.toml"), "user = \"alice\"\n").unwrap();
        fs::create_dir_all(vox.join("cache")).unwrap();
        fs::write(vox.join("cache").join("notes.txt"), "keep me\n").unwrap();
        fs::write(vox.join("user-notes.md"), "# mine\n").unwrap();
    }

    #[test]
    fn strip_removes_only_the_two_line_block_in_the_middle() {
        let before = "\
# user aliases
alias ll='ls -la'

# Added by voxup
export PATH=\"/tmp/fixture/.vox/bin:$PATH\"

# more user content
export EDITOR=vim
";
        let after = strip_voxup_block(before).expect("marker present");
        // Surrounding blank lines are not part of the two-line block and stay.
        let expected = "\
# user aliases
alias ll='ls -la'


# more user content
export EDITOR=vim
";
        assert_eq!(
            after.as_bytes(),
            expected.as_bytes(),
            "every other byte must stay identical\n--- after ---\n{after}\n--- expected ---\n{expected}"
        );
    }

    #[test]
    fn strip_returns_none_when_marker_absent() {
        assert!(strip_voxup_block("export PATH=\"$HOME/.vox/bin:$PATH\"\n").is_none());
    }

    #[test]
    fn parse_active_fails_closed_on_garbage() {
        assert!(parse_active_version("").is_none());
        assert!(parse_active_version("   ").is_none());
        assert!(parse_active_version("../etc").is_none());
        assert!(parse_active_version("0.6.0/../../").is_none());
        assert_eq!(parse_active_version("0.6.0\n").as_deref(), Some("0.6.0"));
    }

    #[test]
    fn prune_fails_closed_when_active_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let tc = tmp.path().join("toolchains");
        fs::create_dir_all(tc.join("vox-0.6.0")).unwrap();
        let err = prune_old_toolchains(&tc, 2).unwrap_err();
        assert!(err.to_string().contains("missing"), "got: {err}");
        assert!(
            tc.join("vox-0.6.0").exists(),
            "must not delete on fail-closed"
        );
    }

    #[test]
    fn prune_fails_closed_when_active_unparseable() {
        let tmp = tempfile::tempdir().unwrap();
        let tc = tmp.path().join("toolchains");
        fs::create_dir_all(tc.join("vox-0.6.0")).unwrap();
        fs::write(tc.join("active"), "not-a-version").unwrap();
        let err = prune_old_toolchains(&tc, 2).unwrap_err();
        assert!(err.to_string().contains("unparseable"), "got: {err}");
        assert!(tc.join("vox-0.6.0").exists());
    }

    #[test]
    fn prune_keeps_active_plus_n() {
        let tmp = tempfile::tempdir().unwrap();
        let tc = tmp.path().join("toolchains");
        for v in ["0.4.0", "0.5.0", "0.6.0", "0.7.0"] {
            fs::create_dir_all(tc.join(format!("vox-{v}"))).unwrap();
        }
        fs::write(tc.join("active"), "0.7.0\n").unwrap();
        let removed = prune_old_toolchains(&tc, 2).unwrap();
        assert!(tc.join("vox-0.7.0").exists());
        assert!(tc.join("vox-0.6.0").exists());
        assert!(tc.join("vox-0.5.0").exists());
        assert!(!tc.join("vox-0.4.0").exists());
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn uninstall_removes_allowlist_and_preserves_user_bytes() {
        let (_tmp, home) = isolated_home();
        let vox = home.join(".vox");
        seed_user_files(&vox);
        let key_before = fs::read(vox.join(".vox-master-key")).unwrap();
        let login_before = fs::read(vox.join("login.toml")).unwrap();
        let cache_before = fs::read(vox.join("cache").join("notes.txt")).unwrap();
        let notes_before = fs::read(vox.join("user-notes.md")).unwrap();

        fs::create_dir_all(vox.join("bin")).unwrap();
        fs::write(vox.join("bin").join("vox"), b"binary").unwrap();
        fs::create_dir_all(vox.join("toolchains").join("vox-0.6.0")).unwrap();
        fs::write(vox.join("toolchains").join("active"), "0.6.0\n").unwrap();
        fs::create_dir_all(vox.join("run")).unwrap();
        fs::write(vox.join("run").join("daemon.pid"), b"1").unwrap();

        let report = uninstall_at(
            &home,
            &UninstallOpts {
                dry_run: false,
                keep_previous: 2,
            },
        )
        .unwrap();

        assert!(!vox.join("bin").exists());
        assert!(!vox.join("toolchains").exists());
        assert!(!vox.join("run").exists());
        assert!(vox.exists(), "~/.vox must still exist");
        assert_eq!(fs::read(vox.join(".vox-master-key")).unwrap(), key_before);
        assert_eq!(fs::read(vox.join("login.toml")).unwrap(), login_before);
        assert_eq!(
            fs::read(vox.join("cache").join("notes.txt")).unwrap(),
            cache_before
        );
        assert_eq!(fs::read(vox.join("user-notes.md")).unwrap(), notes_before);
        assert_eq!(report.removed.len(), 3);
    }

    #[test]
    fn uninstall_dry_run_deletes_nothing() {
        let (_tmp, home) = isolated_home();
        let vox = home.join(".vox");
        seed_user_files(&vox);
        fs::create_dir_all(vox.join("bin")).unwrap();
        fs::write(vox.join("bin").join("vox"), b"binary").unwrap();
        let key_before = fs::read(vox.join(".vox-master-key")).unwrap();

        uninstall_at(
            &home,
            &UninstallOpts {
                dry_run: true,
                keep_previous: 2,
            },
        )
        .unwrap();

        assert!(vox.join("bin").join("vox").exists());
        assert_eq!(fs::read(vox.join(".vox-master-key")).unwrap(), key_before);
    }

    #[test]
    fn profile_block_in_the_middle_is_removed_and_backed_up() {
        let (_tmp, home) = isolated_home();
        let zshrc = home.join(".zshrc");
        let bin = home.join(".vox").join("bin");
        let before = format!(
            "\
# user aliases
alias ll='ls -la'

# Added by voxup
export PATH=\"{}:$PATH\"

# more user content
export EDITOR=vim
",
            bin.display()
        );
        fs::write(&zshrc, &before).unwrap();
        fs::create_dir_all(&bin).unwrap();

        let report = uninstall_at(
            &home,
            &UninstallOpts {
                dry_run: false,
                keep_previous: 2,
            },
        )
        .unwrap();

        let after = fs::read(&zshrc).unwrap();
        let expected = b"\
# user aliases
alias ll='ls -la'


# more user content
export EDITOR=vim
";
        assert_eq!(after, expected.as_slice());
        let ProfileEdit::Removed { backup, .. } = &report.profile_edits[0] else {
            panic!("expected Removed, got {:?}", report.profile_edits);
        };
        assert!(backup.exists(), "backup must be written");
        assert_eq!(fs::read(backup).unwrap(), before.as_bytes());
    }

    #[test]
    fn unmarked_path_line_is_not_edited() {
        let (_tmp, home) = isolated_home();
        let zshrc = home.join(".zshrc");
        let bin = home.join(".vox").join("bin");
        let body = format!("export PATH=\"{}:$PATH\"\n# my stuff\n", bin.display());
        fs::write(&zshrc, &body).unwrap();
        fs::create_dir_all(&bin).unwrap();

        let report = uninstall_at(
            &home,
            &UninstallOpts {
                dry_run: false,
                keep_previous: 2,
            },
        )
        .unwrap();

        assert_eq!(fs::read_to_string(&zshrc).unwrap(), body);
        assert!(
            matches!(report.profile_edits[0], ProfileEdit::Manual { line: 1, .. }),
            "got {:?}",
            report.profile_edits
        );
    }

    #[test]
    fn cargo_vox_left_unless_proven_hardlink() {
        let (_tmp, home) = isolated_home();
        let cargo_bin = home.join(".cargo").join("bin");
        fs::create_dir_all(&cargo_bin).unwrap();
        fs::write(cargo_bin.join("vox"), b"cargo-install-copy").unwrap();
        fs::create_dir_all(home.join(".vox").join("bin")).unwrap();
        fs::write(home.join(".vox").join("bin").join("vox"), b"voxup-copy").unwrap();

        let report = uninstall_at(
            &home,
            &UninstallOpts {
                dry_run: false,
                keep_previous: 2,
            },
        )
        .unwrap();

        assert!(
            cargo_bin.join("vox").exists(),
            "must not delete a cargo-install binary"
        );
        assert!(matches!(
            report.cargo_vox,
            CargoVoxAction::LeftInPlace { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn cargo_vox_removed_when_hardlinked() {
        let (_tmp, home) = isolated_home();
        let bin = home.join(".vox").join("bin");
        let cargo_bin = home.join(".cargo").join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(&cargo_bin).unwrap();
        let canonical = bin.join("vox");
        let cargo = cargo_bin.join("vox");
        fs::write(&canonical, vec![0u8; 80 * 1024]).unwrap();
        fs::hard_link(&canonical, &cargo).unwrap();
        assert!(is_proven_hardlink(&canonical, &cargo));

        uninstall_at(
            &home,
            &UninstallOpts {
                dry_run: false,
                keep_previous: 2,
            },
        )
        .unwrap();

        assert!(!cargo.exists(), "proven hardlink must be removed");
    }

    #[test]
    fn rollback_moves_active_to_previous() {
        let tmp = tempfile::tempdir().unwrap();
        let tc = tmp.path().join("toolchains");
        fs::create_dir_all(tc.join("vox-0.6.0")).unwrap();
        fs::create_dir_all(tc.join("vox-0.7.0")).unwrap();
        fs::write(tc.join("active"), "0.7.0\n").unwrap();
        let next = rollback_active(&tc).unwrap();
        assert_eq!(next, "0.6.0");
        assert_eq!(
            fs::read_to_string(tc.join("active")).unwrap().trim(),
            "0.6.0"
        );
    }

    #[test]
    fn rollback_fails_closed_without_active() {
        let tmp = tempfile::tempdir().unwrap();
        let tc = tmp.path().join("toolchains");
        fs::create_dir_all(&tc).unwrap();
        assert!(rollback_active(&tc).is_err());
    }

    #[test]
    fn dry_run_default_when_stdin_not_tty_is_overridable() {
        let from_flag = UninstallOpts::from_cli(true, false);
        assert!(from_flag.dry_run);
        let apply = UninstallOpts::from_cli(false, true);
        assert!(!apply.dry_run);
    }

    #[test]
    fn civil_iso8601_known_instant() {
        assert_eq!(civil_iso8601(0), "1970-01-01T00:00:00Z");
        // 2026-09-06T18:00:00Z
        assert_eq!(civil_iso8601(1_788_717_600), "2026-09-06T18:00:00Z");
    }

    #[test]
    fn remove_allowlisted_refuses_unknown_name() {
        let tmp = tempfile::tempdir().unwrap();
        let sneaky = tmp.path().join("cache");
        fs::create_dir_all(&sneaky).unwrap();
        let err = remove_allowlisted(&sneaky).unwrap_err();
        assert!(err.to_string().contains("not on the uninstall allowlist"));
        assert!(sneaky.exists());
    }
}
