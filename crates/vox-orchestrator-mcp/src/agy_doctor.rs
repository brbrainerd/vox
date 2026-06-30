//! Detects, locates, and diagnoses the Antigravity `agy` CLI, and produces
//! precise, LLM-followable remediation when it is missing or unauthenticated.
//! Auth is interactive OAuth — we never store credentials; we only instruct.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum AgyStatus {
    Missing,
    PresentUnauthed { path: String },
    Ready { path: String, version: String },
}

/// Best-effort platform install locations the installer documents, in addition to PATH.
pub fn known_install_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(home) = dirs_home() {
        v.push(home.join(".local").join("bin"));
    }
    #[cfg(windows)]
    if let Ok(lad) = std::env::var("LOCALAPPDATA") {
        v.push(PathBuf::from(lad).join("Antigravity"));
    }
    v
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn bin_name() -> &'static str {
    if cfg!(windows) { "agy.exe" } else { "agy" }
}

/// Resolve the binary path via PATH (`which`) then known install dirs.
pub fn resolve_agy() -> Option<PathBuf> {
    if let Ok(p) = which::which("agy") {
        return Some(p);
    }
    for d in known_install_dirs() {
        let candidate = d.join(bin_name());
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Synchronous, fast classification (no long network calls). `version` is probed
/// via `agy --version`; auth state is best-effort (see note in detect()).
pub fn detect() -> AgyStatus {
    let Some(path) = resolve_agy() else {
        return AgyStatus::Missing;
    };
    let path_s = path.to_string_lossy().to_string();
    // vox-arch-check: allow agy-exec  (annotation active once Task 15 adds the rule)
    let ver = std::process::Command::new(&path).arg("--version").output();
    match ver {
        Ok(o) if o.status.success() => AgyStatus::Ready {
            path: path_s,
            version: String::from_utf8_lossy(&o.stdout).trim().to_string(),
        },
        // Binary exists but `--version` failed (could be unauthed first-run, or a
        // different version flag). Treat as present-but-needs-attention; the
        // delegation path surfaces the real stderr on first use.
        _ => AgyStatus::PresentUnauthed { path: path_s },
    }
}

pub fn remediation(status: &AgyStatus) -> String {
    match status {
        AgyStatus::Missing => "`agy` (Antigravity CLI) is not installed or not on PATH.\n\
             INSTALL (verify the URL at https://antigravity.google/docs/cli before running):\n\
             - Unix / Windows-Git-Bash: curl -fsSL https://antigravity.google/cli/install.sh | bash\n\
             The installer drops `agy` into ~/.local/bin (Unix) or %LOCALAPPDATA%\\Antigravity (Windows).\n\
             ADD TO PATH if `agy --version` still fails after install, then restart the shell.\n\
             THEN authenticate (interactive, one-time): run `agy` once and complete the Google Sign-In.\n\
             Re-run vox_agy_doctor to confirm Ready.".to_string(),
        AgyStatus::PresentUnauthed { path } => format!(
            "`agy` was found at {path} but is not confirmed ready (likely needs a one-time \
             interactive Google Sign-In / OAuth, or uses a different version flag).\n\
             ACTION (human, one-time): run `agy` in a terminal and complete the sign-in flow. \
             We do NOT store Google credentials. Then re-run vox_agy_doctor."
        ),
        AgyStatus::Ready { path, version } => {
            format!("`agy` ready at {path} (version: {version}).")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remediation_for_missing_is_actionable() {
        let r = remediation(&AgyStatus::Missing);
        assert!(r.contains("install.sh"));
        assert!(r.contains("agy")); // names the binary
    }

    #[test]
    fn remediation_for_unauthed_mentions_interactive_login() {
        let r = remediation(&AgyStatus::PresentUnauthed {
            path: "/x/agy".into(),
        });
        assert!(r.to_lowercase().contains("sign-in") || r.to_lowercase().contains("oauth"));
    }

    #[test]
    fn known_install_dirs_are_platform_specific() {
        let dirs = known_install_dirs();
        assert!(!dirs.is_empty());
    }
}
