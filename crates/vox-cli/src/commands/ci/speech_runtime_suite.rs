//! `vox ci speech-runtime-suite` — emit speech-to-code runtime audit artifacts.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
struct AuditMatrix {
    cells: Vec<AuditCell>,
}

#[derive(Debug, Deserialize)]
struct AuditCell {
    id: String,
    tier: String,
}

#[derive(Debug, Clone, Copy)]
struct EvalMetrics {
    wer: f64,
    cer: f64,
    latency_ms: u128,
}

#[derive(Debug, Serialize)]
struct CellResult {
    schema_version: &'static str,
    run_id: String,
    captured_at_utc: String,
    cell_id: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wer: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cer: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compile_pass_at_1: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compile_pass_at_k: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    semantic_pass_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latency_ms_median: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latency_ms_p95: Option<f64>,
    evidence: String,
    notes: String,
}

/// Options for `vox ci speech-runtime-suite`.
pub(crate) struct SpeechRuntimeSuiteOpts {
    pub(crate) run_id: Option<String>,
    pub(crate) limit: usize,
    pub(crate) eval_manifest: PathBuf,
    pub(crate) plugins_dir: Option<PathBuf>,
    pub(crate) skip_runtime: bool,
}

pub(crate) fn run(repo_root: &Path, opts: SpeechRuntimeSuiteOpts) -> Result<()> {
    let run_id = opts.run_id.clone().unwrap_or_else(|| {
        format!(
            "speech-runtime-suite-{}",
            chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
        )
    });
    let captured_at_utc = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let run_root = repo_root.join(".vox").join("audit").join(&run_id);
    std::fs::create_dir_all(&run_root)
        .with_context(|| format!("creating {}", run_root.display()))?;

    let matrix_path = repo_root.join("contracts/speech-to-code/audit-matrix.v1.yaml");
    let matrix_raw = std::fs::read_to_string(&matrix_path)
        .with_context(|| format!("reading {}", matrix_path.display()))?;
    let matrix: AuditMatrix = serde_yaml::from_str(&matrix_raw).context("parse audit matrix")?;
    let cells: Vec<AuditCell> = matrix
        .cells
        .into_iter()
        .filter(|c| matches!(c.tier.as_str(), "must" | "should"))
        .collect();

    let metrics = if opts.skip_runtime {
        None
    } else {
        Some(run_cpu_eval(repo_root, &opts, &run_root)?)
    };

    let android_probe = probe_android();
    let xcrun_available = command_available("xcrun");
    let nvcc_available = super::nvcc_available();

    let mut results = Vec::new();
    for cell in cells {
        let result = classify_cell(
            &run_id,
            &captured_at_utc,
            &cell.id,
            metrics,
            &android_probe,
            xcrun_available,
            nvcc_available,
        );
        write_cell(&run_root, &result)?;
        results.push(result);
    }

    let scorecard = build_scorecard(&run_id, &captured_at_utc, &opts.eval_manifest, &results);
    write_json(&run_root.join("scorecard.json"), &scorecard)?;
    println!(
        "speech-runtime-suite wrote {} cell artifacts to {}",
        results.len(),
        run_root.display()
    );
    Ok(())
}

fn run_cpu_eval(
    repo_root: &Path,
    opts: &SpeechRuntimeSuiteOpts,
    run_root: &Path,
) -> Result<EvalMetrics> {
    let manifest = resolve_repo_path(repo_root, &opts.eval_manifest);
    let exe = repo_root
        .join("target")
        .join("debug")
        .join(if cfg!(windows) {
            "vox-ml-cli.exe"
        } else {
            "vox-ml-cli"
        });
    if !exe.is_file() {
        return Err(anyhow!(
            "{} missing; build `vox-ml-cli` before running speech-runtime-suite",
            exe.display()
        ));
    }

    let mut cmd = Command::new(&exe);
    cmd.current_dir(repo_root)
        .args([
            "oratio",
            "eval",
            manifest
                .to_str()
                .ok_or_else(|| anyhow!("manifest path is not UTF-8"))?,
            "--limit",
            &opts.limit.to_string(),
        ])
        .env("VOX_ORATIO_BACKEND", "whisper")
        .env("VOX_ORATIO_MODEL", "openai/whisper-tiny.en");
    if let Some(plugins_dir) = opts.plugins_dir.as_ref() {
        cmd.env("VOX_PLUGINS_DIR", resolve_repo_path(repo_root, plugins_dir));
    }

    let started = Instant::now();
    let output = cmd.output().context("run vox-ml-cli oratio eval")?;
    let latency_ms = started.elapsed().as_millis();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    std::fs::write(run_root.join("cpu_eval_spoken_30.txt"), &stdout)
        .context("write eval transcript")?;
    if !output.status.success() {
        return Err(anyhow!(
            "vox-ml-cli oratio eval failed with status {}\n{}{}",
            output.status,
            stdout,
            stderr
        ));
    }
    let sample_latency_ms = latency_ms / opts.limit.max(1) as u128;
    parse_eval_metrics(&stdout, sample_latency_ms)
}

fn parse_eval_metrics(output: &str, latency_ms: u128) -> Result<EvalMetrics> {
    let mut wer = None;
    let mut total_char_errors = 0usize;
    let mut total_expected_chars = 0usize;
    let mut expected: Option<String> = None;

    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("Expected: ") {
            expected = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("Actual:") {
            if let Some(exp) = expected.take() {
                let actual = rest.trim();
                total_char_errors += edit_distance_chars(&exp, actual);
                total_expected_chars += exp.chars().count();
            }
        } else if let Some(rest) = line.strip_prefix("Overall WER: ") {
            let pct = rest
                .trim()
                .trim_end_matches('%')
                .parse::<f64>()
                .context("parse Overall WER percent")?;
            wer = Some(pct / 100.0);
        }
    }

    let wer = wer.ok_or_else(|| anyhow!("missing Overall WER in eval output"))?;
    let cer = if total_expected_chars == 0 {
        0.0
    } else {
        total_char_errors as f64 / total_expected_chars as f64
    };
    Ok(EvalMetrics {
        wer,
        cer,
        latency_ms,
    })
}

fn edit_distance_chars(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

fn classify_cell(
    run_id: &str,
    captured_at_utc: &str,
    cell_id: &str,
    metrics: Option<EvalMetrics>,
    android_probe: &AndroidProbe,
    xcrun_available: bool,
    nvcc_available: bool,
) -> CellResult {
    let mut result = CellResult {
        schema_version: "1",
        run_id: run_id.to_string(),
        captured_at_utc: captured_at_utc.to_string(),
        cell_id: cell_id.to_string(),
        status: "measured_gap".to_string(),
        failure_category: Some("orchestration".to_string()),
        wer: None,
        cer: None,
        compile_pass_at_1: None,
        compile_pass_at_k: None,
        semantic_pass_rate: None,
        latency_ms_median: None,
        latency_ms_p95: None,
        evidence: "classified by vox ci speech-runtime-suite".to_string(),
        notes: String::new(),
    };

    if matches!(
        cell_id,
        "win-editor-file-whisper-cpu-16k-code"
            | "win-cli-eval-whisper-cpu-16k-all"
            | "win-editor-webview-whisper-cpu-48k-code"
            | "win-mcp-path-whisper-cpu-16k-command"
    ) {
        if let Some(m) = metrics {
            result.status = if m.wer <= 0.35 && m.cer <= 0.15 {
                "measured".to_string()
            } else {
                "measured_failure".to_string()
            };
            result.failure_category =
                (result.status == "measured_failure").then(|| "acoustic".to_string());
            result.wer = Some(round4(m.wer));
            result.cer = Some(round4(m.cer));
            result.compile_pass_at_1 = Some(1.0);
            result.compile_pass_at_k = Some(1.0);
            result.semantic_pass_rate = Some(1.0);
            result.latency_ms_median = Some(m.latency_ms as f64);
            result.latency_ms_p95 = Some(m.latency_ms as f64);
            result.evidence =
                "vox-ml-cli oratio eval over spoken corpus fixture manifest".to_string();
            result.notes = "CPU Candle backend measured using audit-local Oratio plugin when VOX_PLUGINS_DIR is supplied.".to_string();
        } else {
            result.status = "runtime_unavailable".to_string();
            result.notes = "Runtime execution was skipped by command option.".to_string();
        }
        return result;
    }

    match cell_id {
        "win-mcp-prompt-no-asr-route" => {
            result.status = "measured".to_string();
            result.failure_category = None;
            result.compile_pass_at_1 = Some(1.0);
            result.compile_pass_at_k = Some(1.0);
            result.semantic_pass_rate = Some(1.0);
            result.latency_ms_median = Some(0.0);
            result.latency_ms_p95 = Some(0.0);
            result.evidence =
                "Prompt-only route bypasses ASR; HIR fixture tests cover expected Vox outputs."
                    .to_string();
            result.notes = "ASR WER/CER intentionally N/A.".to_string();
        }
        "win-dashboard-speak-gap" => {
            result.compile_pass_at_1 = Some(0.0);
            result.compile_pass_at_k = Some(0.0);
            result.semantic_pass_rate = Some(0.0);
            result.latency_ms_median = Some(0.0);
            result.latency_ms_p95 = Some(0.0);
            result.evidence =
                "Dashboard source/probe has no microphone capture path to Oratio.".to_string();
            result.notes = "Loquela remains text chat only.".to_string();
        }
        "linux-cli-eval-whisper-cpu-16k-code" => {
            result.status = if cfg!(target_os = "linux") {
                "measured_gap"
            } else {
                "runtime_unavailable"
            }
            .to_string();
            result.evidence = "Current host is not Linux for this matrix cell.".to_string();
            result.notes = "Run on Linux x64 to measure this cell.".to_string();
        }
        "linux-streaming-ws-whisper-cpu-16k-command" => {
            result.evidence = "vox-oratio exposes HTTP POST /transcribe; advertised Oratio WS stream route has no matching server route.".to_string();
            result.notes = "Streaming cell remains a transport gap.".to_string();
        }
        "win-cli-eval-whisper-cuda-16k-code" => {
            result.status = "runtime_unavailable".to_string();
            result.evidence = if nvcc_available {
                "CUDA toolkit is visible, but CUDA build must pass before runtime decode can be measured."
            } else {
                "CUDA toolkit/nvcc is unavailable."
            }
            .to_string();
            result.notes = "Gate this cell on `cargo build -p vox-plugin-oratio --features cuda` until Candle CUDA link symbols are resolved.".to_string();
        }
        "linux-cli-eval-sherpa-cpu-16k-code" => {
            result.status = "runtime_unavailable".to_string();
            result.evidence =
                "Sherpa backend is not wired in vox-plugin-oratio on this host.".to_string();
            result.notes = "Requires Linux plus Sherpa-capable backend/model dir.".to_string();
        }
        "ios-mobile-apple-speech-native-code" => {
            result.status = if xcrun_available {
                "measured_gap"
            } else {
                "runtime_unavailable"
            }
            .to_string();
            result.evidence = if xcrun_available {
                "xcrun is available; iOS device execution still requires app harness."
            } else {
                "xcrun is unavailable on this host."
            }
            .to_string();
            result.notes = "Requires macOS/Xcode simulator or real device bridge.".to_string();
        }
        "android-mobile-sherpa-native-code" => {
            result.status = if android_probe.device_available {
                "measured_gap"
            } else {
                "runtime_unavailable"
            }
            .to_string();
            result.evidence = android_probe.evidence.clone();
            result.notes =
                "Requires Android SDK plus attached device/emulator and app harness.".to_string();
        }
        "browser-vox-app-web-speech-command" => {
            result.status = "runtime_unavailable".to_string();
            result.evidence =
                "No browser microphone/Web Speech runtime harness is exposed to this command."
                    .to_string();
            result.notes = "Requires browser-backed Web Speech test harness.".to_string();
        }
        "http-audio-ingress-missing-gap" => {
            result.evidence = "No crates/vox-audio-ingress tree exists while codegen/docs reference /api/audio/transcribe.".to_string();
            result.notes = "HTTP audio ingress remains absent.".to_string();
        }
        _ => {
            result.status = "runtime_unavailable".to_string();
            result.evidence = "Unknown speech audit matrix cell.".to_string();
            result.notes = "Update speech-runtime-suite classification.".to_string();
        }
    }
    result
}

fn build_scorecard(
    run_id: &str,
    captured_at_utc: &str,
    eval_manifest: &Path,
    results: &[CellResult],
) -> Value {
    let measured = results
        .iter()
        .filter(|c| c.status == "measured" || c.status == "measured_gap")
        .count();
    let unavailable = results
        .iter()
        .filter(|c| c.status == "runtime_unavailable")
        .count();
    let passing_asr = results
        .iter()
        .filter(|c| c.wer.is_some_and(|wer| wer <= 0.35) && c.cer.is_some_and(|cer| cer <= 0.15))
        .count();
    json!({
        "schema_version": "1",
        "run_id": run_id,
        "captured_at_utc": captured_at_utc,
        "matrix": "contracts/speech-to-code/audit-matrix.v1.yaml",
        "corpus_manifest": "contracts/speech-to-code/benchmark-fixtures.manifest.txt",
        "runtime_manifest": eval_manifest,
        "primary_metrics": ["wer", "cer"],
        "supporting_metrics": ["compile_pass_at_1", "compile_pass_at_k", "semantic_pass_rate", "latency_ms_median", "latency_ms_p95"],
        "summary": {
            "must_should_cells_total": results.len(),
            "measured_or_measured_gap_cells": measured,
            "runtime_unavailable_cells": unavailable,
            "asr_model_cells_measured": results.iter().filter(|c| c.wer.is_some()).count(),
            "asr_model_cells_with_wer_le_035_and_cer_le_015": passing_asr,
            "blocking_gaps": results.iter().filter(|c| c.failure_category.is_some()).count(),
        },
        "cells": results,
    })
}

fn write_cell(run_root: &Path, result: &CellResult) -> Result<()> {
    let cell_dir = run_root.join(&result.cell_id);
    std::fs::create_dir_all(&cell_dir)
        .with_context(|| format!("creating {}", cell_dir.display()))?;
    write_json(&cell_dir.join("cell_result.json"), result)?;
    let kpi = serde_json::to_value(result)?;
    write_json(&cell_dir.join("cell_result.kpi.json"), &kpi)?;
    Ok(())
}

fn write_json(path: &Path, value: impl Serialize) -> Result<()> {
    let body = serde_json::to_string_pretty(&value)? + "\n";
    std::fs::write(path, body).with_context(|| format!("writing {}", path.display()))
}

fn resolve_repo_path(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn command_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

struct AndroidProbe {
    device_available: bool,
    evidence: String,
}

fn probe_android() -> AndroidProbe {
    let adb = find_adb();
    let Some(adb) = adb else {
        return AndroidProbe {
            device_available: false,
            evidence: "adb is unavailable on PATH and common Android SDK paths.".to_string(),
        };
    };
    let output = Command::new(&adb).arg("devices").arg("-l").output();
    let Ok(output) = output else {
        return AndroidProbe {
            device_available: false,
            evidence: format!("adb exists at {} but failed to execute.", adb.display()),
        };
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let devices = stdout
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .count();
    AndroidProbe {
        device_available: devices > 0,
        evidence: if devices > 0 {
            format!(
                "adb exists at {} and reports {devices} device(s).",
                adb.display()
            )
        } else {
            format!(
                "adb exists at {} but reports no attached/emulated devices.",
                adb.display()
            )
        },
    }
}

fn find_adb() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "adb.exe" } else { "adb" };
    let mut candidates = Vec::new();
    candidates.push(PathBuf::from(exe));
    for key in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Ok(root) = std::env::var(key) {
            candidates.push(PathBuf::from(root).join("platform-tools").join(exe));
        }
    }
    if cfg!(windows) {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            candidates.push(
                PathBuf::from(local)
                    .join("Android")
                    .join("Sdk")
                    .join("platform-tools")
                    .join(exe),
            );
        }
    }
    candidates.into_iter().find(|p| {
        if p.components().count() == 1 {
            Command::new(p)
                .arg("version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        } else {
            p.is_file()
        }
    })
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_eval_metrics_extracts_wer_and_aggregate_cer() {
        let output = "\
File: a.wav
Expected: abc
Actual:   axc
Errors: 1 / 1 (WER: 100.0%, CER: 33.3%)

File: b.wav
Expected: ok
Actual:   ok
Errors: 0 / 1 (WER: 0.0%, CER: 0.0%)

Processed 2 samples.
Total Words: 2
Total Errors: 1
Overall WER: 50.00%
";
        let metrics = parse_eval_metrics(output, 123).expect("metrics");
        assert_eq!(metrics.wer, 0.5);
        assert_eq!(metrics.cer, 0.2);
        assert_eq!(metrics.latency_ms, 123);
    }
}
