//! Persist `~/.vox/bin` in the user's shell profile files.
//! Public function: [`add_to_path`]. Idempotent.

use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

pub fn add_to_path(home: &Path, bin_dir: &Path) -> Vec<PathBuf> {
    let mut modified = Vec::new();
    #[cfg(windows)]
    {
        add_to_windows_registry_path(bin_dir);
    }
    for name in &[".bashrc", ".bash_profile", ".zshrc", ".profile"] {
        let p = home.join(name);
        if try_append(&p, &posix_snippet(bin_dir), bin_dir) {
            modified.push(p);
        }
    }
    let fish = home.join(".config").join("fish").join("config.fish");
    let fish_exists = fish.exists();
    if try_append(&fish, &fish_snippet(bin_dir), bin_dir) {
        modified.push(fish);
    }

    // `try_append` skips profiles that don't exist. A pristine macOS account has
    // none of them — macOS ships no default `~/.zshrc` even though zsh is the
    // login shell — so every append above bails and voxup silently puts
    // `~/.vox/bin` on no PATH at all, with `vox` missing after a "successful"
    // install. Create the profile for the login shell instead of giving up.
    // Gate on "no profile exists at all", not "nothing was modified" — a repeat
    // run finds its own entry already present and modifies nothing, which must
    // not be mistaken for the pristine case.
    #[cfg(unix)]
    if [".bashrc", ".bash_profile", ".zshrc", ".profile"]
        .iter()
        .all(|n| !home.join(n).exists())
        && !fish_exists
    {
        let shell = std::env::var("SHELL").unwrap_or_default();
        let shell_name = Path::new(&shell)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (profile, snippet) = match shell_name.as_str() {
            "fish" => (
                home.join(".config").join("fish").join("config.fish"),
                fish_snippet(bin_dir),
            ),
            "bash" => (home.join(".bashrc"), posix_snippet(bin_dir)),
            // zsh is the macOS default; `.profile` is the portable fallback.
            "zsh" => (home.join(".zshrc"), posix_snippet(bin_dir)),
            _ => (home.join(".profile"), posix_snippet(bin_dir)),
        };
        if create_and_append(&profile, &snippet) {
            modified.push(profile);
        }
    }
    if let Some(docs) = ps_documents_dir(home) {
        let ps = ps_snippet(bin_dir);
        for sub in &[
            "PowerShell/Microsoft.PowerShell_profile.ps1",
            "WindowsPowerShell/Microsoft.PowerShell_profile.ps1",
        ] {
            let p = docs.join(sub);
            if try_append(&p, &ps, bin_dir) {
                modified.push(p);
            }
        }
    }
    modified
}

#[cfg(windows)]
fn add_to_windows_registry_path(bin_dir: &Path) {
    use std::process::Command;
    let path_str = bin_dir.to_string_lossy().to_string();
    let cmd = format!(
        "$old = [Environment]::GetEnvironmentVariable('PATH', [System.EnvironmentVariableTarget]::User); \
         if ($old -split ';' -notcontains '{}') {{ \
             $new = if ([string]::IsNullOrEmpty($old)) {{ '{}' }} else {{ \"$old;{}\" }}; \
             [Environment]::SetEnvironmentVariable('PATH', $new, [System.EnvironmentVariableTarget]::User); \
         }}",
        path_str.replace("'", "''"),
        path_str.replace("'", "''"),
        path_str.replace("'", "''")
    );
    // Already inside `#[cfg(windows)] fn add_to_windows_registry_path` — the spawn
    // is platform-gated as the rule requires; the regex just can't see the cfg.
    // vox-arch-check: allow shell-spawn
    let _ = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(&cmd)
        .status();
}

fn posix_path(bin_dir: &Path) -> String {
    let s = bin_dir.to_string_lossy().replace('\\', "/");
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        let drive = (s.as_bytes()[0] as char).to_ascii_lowercase();
        format!("/{drive}{}", &s[2..])
    } else {
        s
    }
}

fn posix_snippet(bin_dir: &Path) -> String {
    format!(
        "\n# Added by voxup\nexport PATH=\"{}:$PATH\"\n",
        posix_path(bin_dir)
    )
}

fn fish_snippet(bin_dir: &Path) -> String {
    format!(
        "\n# Added by voxup\nfish_add_path \"{}\"\n",
        posix_path(bin_dir)
    )
}

fn ps_snippet(bin_dir: &Path) -> String {
    format!(
        "\n# Added by voxup\n$env:PATH = \"{};$env:PATH\"\n",
        bin_dir.display()
    )
}

/// Create `profile` (and any missing parent directories) and write `snippet` to it.
///
/// Only called when no existing profile could be updated — see `add_to_path`.
#[cfg(unix)]
fn create_and_append(profile: &Path, snippet: &str) -> bool {
    if let Some(parent) = profile.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        warn!("Cannot create {}: {e}", parent.display());
        return false;
    }
    match fs::write(profile, snippet) {
        Ok(()) => {
            info!("Created {} with voxup PATH entry", profile.display());
            true
        }
        Err(e) => {
            warn!("Cannot write {}: {e}", profile.display());
            false
        }
    }
}

fn try_append(profile: &Path, snippet: &str, bin_dir: &Path) -> bool {
    if !profile.exists() {
        return false;
    }
    let existing = match fs::read_to_string(profile) {
        Ok(s) => s,
        Err(e) => {
            warn!("Cannot read {}: {e}", profile.display());
            return false;
        }
    };
    let posix = posix_path(bin_dir);
    if existing.contains(&posix) || existing.contains(&*bin_dir.to_string_lossy()) {
        info!("{} already has voxup PATH entry", profile.display());
        return false;
    }
    match fs::write(profile, format!("{existing}{snippet}")) {
        Ok(()) => {
            info!("Updated {}", profile.display());
            true
        }
        Err(e) => {
            warn!("Cannot write {}: {e}", profile.display());
            false
        }
    }
}

fn ps_documents_dir(home: &Path) -> Option<PathBuf> {
    if cfg!(windows) {
        Some(home.join("Documents"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn adds_path_to_existing_bashrc() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        let bashrc = home.join(".bashrc");
        fs::write(&bashrc, "# existing\n").unwrap();
        let bin = home.join(".vox").join("bin");
        let modified = add_to_path(home, &bin);
        assert!(modified.contains(&bashrc));
        let content = fs::read_to_string(&bashrc).unwrap();
        assert!(content.contains(&posix_path(&bin)));
        assert!(content.contains("export PATH="));
    }

    #[test]
    fn is_idempotent_on_second_call() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        let bashrc = home.join(".bashrc");
        fs::write(&bashrc, "# existing\n").unwrap();
        let bin = home.join(".vox").join("bin");
        let first = add_to_path(home, &bin);
        let content_after_first = fs::read_to_string(&bashrc).unwrap();
        let second = add_to_path(home, &bin);
        let content_after_second = fs::read_to_string(&bashrc).unwrap();
        assert!(!first.is_empty());
        assert!(second.is_empty());
        assert_eq!(content_after_first, content_after_second);
    }

    #[test]
    fn try_append_skips_missing_profiles() {
        let tmp = tempdir().unwrap();
        let bin = tmp.path().join(".vox").join("bin");
        let bashrc = tmp.path().join(".bashrc");
        // `try_append` never creates a profile. Bootstrapping one when the account
        // has none is `add_to_path`'s job — see
        // `creates_login_shell_profile_when_none_exist`, which covers the pristine
        // macOS case this used to (incorrectly) assert was a no-op.
        assert!(!try_append(&bashrc, "# x\n", &bin));
        assert!(!bashrc.exists());
    }

    #[test]
    fn adds_fish_config_when_present() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        let fish_conf = home.join(".config").join("fish");
        fs::create_dir_all(&fish_conf).unwrap();
        let fish_file = fish_conf.join("config.fish");
        fs::write(&fish_file, "# fish\n").unwrap();
        let bin = home.join(".vox").join("bin");
        let modified = add_to_path(home, &bin);
        assert!(modified.contains(&fish_file));
        assert!(
            fs::read_to_string(&fish_file)
                .unwrap()
                .contains("fish_add_path")
        );
    }

    #[test]
    fn posix_snippet_format() {
        let bin = PathBuf::from("/home/user/.vox/bin");
        let s = posix_snippet(&bin);
        assert!(s.contains("export PATH=\"/home/user/.vox/bin:$PATH\""));
        assert!(s.contains("# Added by voxup"));
    }

    #[test]
    fn test_posix_path_conversion() {
        assert_eq!(
            posix_path(Path::new("C:\\Users\\Owner\\.vox\\bin")),
            "/c/Users/Owner/.vox/bin"
        );
        assert_eq!(posix_path(Path::new("d:\\path\\to\\bin")), "/d/path/to/bin");
        assert_eq!(posix_path(Path::new("/usr/local/bin")), "/usr/local/bin");
    }

    /// A pristine macOS account has no `~/.zshrc` (macOS ships none), and no
    /// `.bashrc`/`.bash_profile`/`.profile` either. Every `try_append` bails on a
    /// missing file, so without a fallback voxup reports success while adding
    /// `~/.vox/bin` to no profile at all and `vox` stays off PATH.
    #[cfg(unix)]
    #[test]
    fn creates_login_shell_profile_when_none_exist() {
        let home = tempfile::tempdir().expect("tempdir");
        let bin_dir = home.path().join(".vox/bin");

        // Sanity: this is the pristine case — nothing for try_append to find.
        for name in [".bashrc", ".bash_profile", ".zshrc", ".profile"] {
            assert!(!home.path().join(name).exists());
        }

        let prev = std::env::var_os("SHELL");
        // SAFETY: single-threaded test; restored below.
        unsafe { std::env::set_var("SHELL", "/bin/zsh") };
        let modified = add_to_path(home.path(), &bin_dir);
        match prev {
            Some(v) => unsafe { std::env::set_var("SHELL", v) },
            None => unsafe { std::env::remove_var("SHELL") },
        }

        let zshrc = home.path().join(".zshrc");
        assert!(
            zshrc.exists(),
            "voxup must create the login shell's profile when none exists"
        );
        assert!(
            modified.contains(&zshrc),
            "created profile must be reported"
        );
        let body = std::fs::read_to_string(&zshrc).expect("read .zshrc");
        assert!(
            body.contains(&posix_path(&bin_dir)),
            "profile must put the bin dir on PATH, got: {body}"
        );
    }
}
