//! PowerShell backend: structured JSON-friendly probes via `pwsh`.

use std::process::Command;

/// Runs a PowerShell script fragment with `-NoProfile`. Returns (stdout, stderr, exit_code).
pub fn run_pwsh_capture(script: &str) -> std::io::Result<(String, String, Option<i32>)> {
    let output = Command::new("pwsh")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()?;
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
    fn pwsh_json_probe_smoke() {
        if Command::new("pwsh").arg("-v").status().is_err() {
            return;
        }
        let (out, err, code) =
            run_pwsh_capture("'@{ ok = $true }' | ConvertTo-Json -Compress").expect("pwsh");
        assert!(code == Some(0), "stderr={err} out={out}");
        assert!(out.contains("ok"));
    }
}
