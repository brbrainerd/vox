//! Nushell backend (optional): `nu -c` when installed.

use std::process::Command;

/// Runs a Nushell expression via `nu -c`. Returns (stdout, stderr, exit_code).
pub fn run_nu_capture(script: &str) -> std::io::Result<(String, String, Option<i32>)> {
    let output = Command::new("nu").args(["-c", script]).output()?;
    let code = output.status.code();
    Ok((
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        code,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nu_version_smoke() {
        if Command::new("nu").arg("--version").status().is_err() {
            return;
        }
        let (out, err, code) = run_nu_capture("version").expect("nu");
        assert!(code == Some(0), "stderr={err}");
        assert!(!out.is_empty());
    }
}
