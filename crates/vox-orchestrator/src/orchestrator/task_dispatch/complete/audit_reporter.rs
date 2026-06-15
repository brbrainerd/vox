use std::path::PathBuf;

use crate::process_util::quiet_tokio_command;

pub async fn verify_broken_links(write_files: &[PathBuf]) -> String {
    let mut broken_links_report = String::new();
    let md_files: Vec<_> = write_files
        .iter()
        .filter(|p| p.extension().is_some_and(|ext| ext == "md"))
        .collect();

    if !md_files.is_empty() {
        let mut broken_reports = Vec::new();
        for md_file in md_files {
            let out = quiet_tokio_command("cargo")
                .arg("run")
                .arg("-p")
                .arg("vox-cli")
                .arg("--")
                .arg("ci")
                .arg("check-links")
                .arg("--target")
                .arg(md_file)
                .output()
                .await;
            if let Ok(out) = out {
                if !out.status.success() {
                    broken_reports.push(String::from_utf8_lossy(&out.stdout).to_string());
                }
            }
        }
        if !broken_reports.is_empty() {
            broken_links_report = broken_reports.join("\n");
        }
    }
    broken_links_report
}
