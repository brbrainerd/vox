//! Hermetic proxy execution. Replaces the current process with the real vox.

use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use tracing::info;

pub fn hermetic_path_prefix(home: &std::path::Path) -> PathBuf {
    home.join(".vox").join("toolchains").join("bin")
}

pub fn resolve_vox_bin(home: &std::path::Path) -> PathBuf {
    let exe = if cfg!(windows) { "vox.exe" } else { "vox" };
    let tc_dir = home.join(".vox").join("toolchains");

    // 1. Try reading the active version file
    if let Ok(active_ver) = std::fs::read_to_string(tc_dir.join("active")) {
        let path = tc_dir.join(format!("vox-{}", active_ver.trim())).join(exe);
        if path.exists() {
            return path;
        }
    }

    // 2. Fallback: Scan toolchains directory for highest semver
    if let Ok(entries) = std::fs::read_dir(&tc_dir) {
        let mut versions = Vec::new();
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with("vox-") {
                    let ver_str = &name[4..];
                    if let Ok(ver) = semver::Version::parse(ver_str) {
                        versions.push((ver, entry.path()));
                    }
                }
            }
        }
        versions.sort_by(|a, b| b.0.cmp(&a.0));
        if let Some((_, path)) = versions.first() {
            let bin_path = path.join(exe);
            if bin_path.exists() {
                return bin_path;
            }
        }
    }

    // 3. Fallback to ~/.vox/bin/vox (legacy)
    home.join(".vox").join("bin").join(exe)
}

pub async fn run_proxy(args: &[String]) -> Result<()> {
    let home = dirs::home_dir().context("cannot determine home directory")?;
    let vox = resolve_vox_bin(&home);
    if !vox.exists() {
        bail!(
            "vox binary not found at {}. Run `voxup install` first.",
            vox.display()
        );
    }
    let prefix = hermetic_path_prefix(&home);
    let old_path = std::env::var("PATH").unwrap_or_default();
    let sep = if cfg!(windows) { ";" } else { ":" };
    let new_path = if old_path.is_empty() {
        prefix.display().to_string()
    } else {
        format!("{}{sep}{old_path}", prefix.display())
    };
    info!("Proxying to {}", vox.display());
    exec_replace(&vox, args, &new_path)
}

#[cfg(unix)]
fn exec_replace(vox: &std::path::Path, args: &[String], new_path: &str) -> Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_vox = CString::new(vox.as_os_str().as_bytes()).context("vox path contains null byte")?;
    let mut c_args: Vec<CString> = Vec::with_capacity(args.len() + 1);
    c_args.push(c_vox.clone());
    for a in args {
        c_args.push(CString::new(a.as_str()).context("arg contains null byte")?);
    }
    let c_argv: Vec<*const libc::c_char> = c_args
        .iter()
        .map(|s| s.as_ptr())
        .chain(std::iter::once(std::ptr::null()))
        .collect();

    // Construct envp with the new PATH to avoid std::env::set_var UB
    let mut c_envs = Vec::new();
    for (key, val) in std::env::vars() {
        if key != "PATH" {
            let env_str = format!("{key}={val}");
            c_envs.push(CString::new(env_str).context("env contains null byte")?);
        }
    }
    c_envs.push(CString::new(format!("PATH={new_path}")).unwrap());
    let c_envp: Vec<*const libc::c_char> = c_envs
        .iter()
        .map(|s| s.as_ptr())
        .chain(std::iter::once(std::ptr::null()))
        .collect();

    let ret = unsafe { libc::execve(c_vox.as_ptr(), c_argv.as_ptr(), c_envp.as_ptr()) };
    bail!("execve returned {ret}: {}", std::io::Error::last_os_error());
}

#[cfg(windows)]
fn exec_replace(vox: &std::path::Path, args: &[String], new_path: &str) -> Result<()> {
    let status = std::process::Command::new(vox)
        .args(args)
        .env("PATH", new_path)
        .status()
        .with_context(|| format!("spawn {}", vox.display()))?;
    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn hermetic_path_prefix_is_under_home() {
        let home = PathBuf::from("/home/ada");
        assert_eq!(
            hermetic_path_prefix(&home),
            PathBuf::from("/home/ada/.vox/toolchains/bin")
        );
    }

    #[test]
    fn resolve_vox_bin_points_into_dot_vox() {
        let home = PathBuf::from("/home/ada");
        let vox = resolve_vox_bin(&home);
        assert!(
            vox.starts_with("/home/ada/.vox/bin/"),
            "got: {}",
            vox.display()
        );
        assert!(
            vox.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("vox")
        );
    }

    #[test]
    fn test_resolve_vox_bin_reads_active_file() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let tc_dir = home.join(".vox").join("toolchains");
        std::fs::create_dir_all(tc_dir.join("vox-0.7.1")).unwrap();
        std::fs::write(tc_dir.join("active"), "0.7.1").unwrap();

        let exe = if cfg!(windows) { "vox.exe" } else { "vox" };
        let real_bin = tc_dir.join("vox-0.7.1").join(exe);
        std::fs::write(&real_bin, b"placeholder").unwrap();

        let resolved = resolve_vox_bin(home);
        assert_eq!(resolved, real_bin);
    }

    #[test]
    fn test_resolve_vox_bin_scans_highest_semver() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let tc_dir = home.join(".vox").join("toolchains");
        std::fs::create_dir_all(tc_dir.join("vox-0.6.0")).unwrap();
        std::fs::create_dir_all(tc_dir.join("vox-0.7.2")).unwrap();
        std::fs::create_dir_all(tc_dir.join("vox-0.7.10")).unwrap();

        let exe = if cfg!(windows) { "vox.exe" } else { "vox" };
        let real_bin = tc_dir.join("vox-0.7.10").join(exe);
        std::fs::write(&real_bin, b"placeholder").unwrap();

        let resolved = resolve_vox_bin(home);
        assert_eq!(resolved, real_bin);
    }
}
