//! Persist `~/.vox/bin` in the user's shell profile files.
//! Public function: [`add_to_path`]. Idempotent.

use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

pub fn add_to_path(home: &Path, bin_dir: &Path) -> Vec<PathBuf> {
    let mut modified = Vec::new();
    for name in &[".bashrc", ".bash_profile", ".zshrc", ".profile"] {
        let p = home.join(name);
        if try_append(&p, &posix_snippet(bin_dir), bin_dir) {
            modified.push(p);
        }
    }
    let fish = home.join(".config").join("fish").join("config.fish");
    if try_append(&fish, &fish_snippet(bin_dir), bin_dir) {
        modified.push(fish);
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

fn posix_snippet(bin_dir: &Path) -> String {
    format!("\n# Added by voxup\nexport PATH=\"{}:$PATH\"\n", bin_dir.display())
}

fn fish_snippet(bin_dir: &Path) -> String {
    format!("\n# Added by voxup\nfish_add_path \"{}\"\n", bin_dir.display())
}

fn ps_snippet(bin_dir: &Path) -> String {
    format!("\n# Added by voxup\n$env:PATH = \"{};$env:PATH\"\n", bin_dir.display())
}

fn try_append(profile: &Path, snippet: &str, bin_dir: &Path) -> bool {
    if !profile.exists() { return false; }
    let existing = match fs::read_to_string(profile) {
        Ok(s) => s,
        Err(e) => { warn!("Cannot read {}: {e}", profile.display()); return false; }
    };
    if existing.contains(&*bin_dir.to_string_lossy()) {
        info!("{} already has voxup PATH entry", profile.display());
        return false;
    }
    match fs::write(profile, format!("{existing}{snippet}")) {
        Ok(()) => { info!("Updated {}", profile.display()); true }
        Err(e) => { warn!("Cannot write {}: {e}", profile.display()); false }
    }
}

fn ps_documents_dir(home: &Path) -> Option<PathBuf> {
    if cfg!(windows) { Some(home.join("Documents")) } else { None }
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
        assert!(content.contains(&bin.display().to_string()));
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
    fn skips_missing_profiles() {
        let tmp = tempdir().unwrap();
        let bin = tmp.path().join(".vox").join("bin");
        assert!(add_to_path(tmp.path(), &bin).is_empty());
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
        assert!(fs::read_to_string(&fish_file).unwrap().contains("fish_add_path"));
    }

    #[test]
    fn posix_snippet_format() {
        let bin = PathBuf::from("/home/user/.vox/bin");
        let s = posix_snippet(&bin);
        assert!(s.contains("export PATH=\"/home/user/.vox/bin:$PATH\""));
        assert!(s.contains("# Added by voxup"));
    }
}
