//! Hermetic proxy execution. Replaces the current process with the real vox.

use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use tracing::info;

pub fn hermetic_path_prefix(home: &std::path::Path) -> PathBuf {
    home.join(".vox").join("toolchains").join("bin")
}

pub fn resolve_vox_bin(home: &std::path::Path) -> PathBuf {
    let exe = if cfg!(windows) { "vox.exe" } else { "vox" };
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
    // SAFETY: voxup proxy is single-threaded at this call site.
    unsafe { std::env::set_var("PATH", new_path) };
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
    let ret = unsafe { libc::execv(c_vox.as_ptr(), c_argv.as_ptr()) };
    bail!("execv returned {ret}: {}", std::io::Error::last_os_error());
}

#[cfg(windows)]
fn exec_replace(vox: &std::path::Path, args: &[String], new_path: &str) -> Result<()> {
    unsafe { std::env::set_var("PATH", new_path) };
    let status = std::process::Command::new(vox)
        .args(args)
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
}
