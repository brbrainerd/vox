use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Uses the `ffmpeg` CLI (if installed) to extract mono 16kHz f32le audio
/// from a video container. This runs as a subprocess.
pub fn extract_audio_ffmpeg(path: &Path) -> Result<Vec<f32>> {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-v")
        .arg("error")
        .arg("-nostdin")
        .arg("-i")
        .arg(path)
        .arg("-vn") // no video
        .arg("-ac")
        .arg("1") // mono
        .arg("-ar")
        .arg("16000") // 16kHz
        .arg("-f")
        .arg("f32le") // 32-bit float little-endian
        .arg("-"); // output to stdout

    let output = cmd
        .output()
        .context("Failed to launch ffmpeg subprocess. Is ffmpeg in PATH?")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ffmpeg failed with status {}: {}", output.status, stderr);
    }

    // Interpret raw bytes as little-endian f32 samples.
    let pcm: Vec<f32> = output
        .stdout
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();

    Ok(pcm)
}
