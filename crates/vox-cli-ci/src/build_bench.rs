//! `vox ci build-bench` — reproducible wall-clock build scenarios with a
//! committed baseline and phase-delta reporting. Separate from `build-timings`
//! (which carries soft-budget + telemetry semantics): this command's only job is
//! to measure a pinned scenario set, snapshot it, and diff snapshots so each
//! optimization phase can report a real before/after delta.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, Result};

// ── cargo --timings HTML ingest ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingEntry {
    pub name: String,
    pub duration_ms: u64,
}

/// Parse the embedded UNIT_DATA JS array from a cargo --timings HTML file.
///
/// cargo emits `const UNIT_DATA = [ {..}, .. ];` where each entry's `duration`
/// is **float seconds** (e.g. `9.37`). We convert to whole milliseconds and
/// drop zero-duration units (cached / no-rebuild). We do NOT filter on `mode`:
/// in a cached/sccache build the real compile rows carry `mode:"todo"` with a
/// nonzero duration, so a mode filter would discard exactly the data we want.
pub fn extract_unit_data(html: &str) -> Vec<TimingEntry> {
    // Accept either `const`/`var`/`let UNIT_DATA = [` — anchor on the assignment.
    let marker = "UNIT_DATA = [";
    let start = match html.find(marker) {
        Some(i) => i + marker.len() - 1, // include the '['
        None => return vec![],
    };
    let slice = &html[start..];
    // The array closes at the first `];` after the opening `[`.
    let end = match slice.find("];") {
        Some(i) => i + 1,
        None => return vec![],
    };
    let array_json = &slice[..end];
    let raw: Vec<serde_json::Value> = match serde_json::from_str(array_json) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    raw.into_iter()
        .filter_map(|v| {
            // duration is float seconds; convert to whole ms.
            let secs = v["duration"].as_f64()?;
            let duration_ms = (secs * 1000.0).round() as u64;
            if duration_ms == 0 {
                return None; // cached / no-rebuild unit
            }
            let name = v["name"].as_str()?.to_string();
            Some(TimingEntry { name, duration_ms })
        })
        .collect()
}

/// Return the most-recently-modified `cargo-timing-*.html` under `target/cargo-timings/`.
pub fn find_newest_timings_html(repo_root: &Path) -> Option<std::path::PathBuf> {
    let dir = repo_root.join("target/cargo-timings");
    let entries = std::fs::read_dir(&dir).ok()?;
    entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name().to_string_lossy().starts_with("cargo-timing-")
                && e.path().extension().is_some_and(|x| x == "html")
        })
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub run_id: String,
    pub entries: Vec<TimingEntry>,
}

/// Append one JSONL record to the history file (create if absent).
pub fn append_to_history(history_path: &Path, run_id: &str, entries: &[TimingEntry]) -> Result<()> {
    use std::io::Write;
    if let Some(parent) = history_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let record = HistoryRecord {
        run_id: run_id.to_string(),
        entries: entries.to_vec(),
    };
    let line = serde_json::to_string(&record)? + "\n";
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_path)
        .with_context(|| format!("open history {}", history_path.display()))?;
    f.write_all(line.as_bytes())
        .with_context(|| "write history line")?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Scenario {
    pub id: String,
    pub touch: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScenarioFile {
    pub schema_version: u32,
    pub scenarios: Vec<Scenario>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BenchRecord {
    pub id: String,
    pub ok: bool,
    pub wall_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Snapshot {
    pub schema_version: u32,
    pub label: String,
    pub records: Vec<BenchRecord>,
}

impl Snapshot {
    pub fn by_id(&self) -> BTreeMap<&str, &BenchRecord> {
        self.records.iter().map(|r| (r.id.as_str(), r)).collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeltaRow {
    pub id: String,
    pub base_ms: u128,
    pub new_ms: u128,
    pub delta_ms: i128,
    pub pct: f64,
}

pub fn compute_deltas(base: &Snapshot, new: &Snapshot) -> Vec<DeltaRow> {
    let b = base.by_id();
    let mut rows = Vec::new();
    for nr in &new.records {
        if !nr.ok {
            continue;
        }
        if let Some(br) = b.get(nr.id.as_str()) {
            if !br.ok {
                continue;
            }
            let delta = nr.wall_ms as i128 - br.wall_ms as i128;
            let pct = if br.wall_ms > 0 {
                (delta as f64) / (br.wall_ms as f64) * 100.0
            } else {
                0.0
            };
            rows.push(DeltaRow {
                id: nr.id.clone(),
                base_ms: br.wall_ms,
                new_ms: nr.wall_ms,
                delta_ms: delta,
                pct,
            });
        }
    }
    rows
}

pub fn format_delta_markdown(label: &str, rows: &[DeltaRow]) -> String {
    let mut out =
        format!("### {label}\n\n| Scenario | Base | New | Δ | Δ% |\n|---|--:|--:|--:|--:|\n");
    for r in rows {
        let sign = if r.delta_ms <= 0 { "" } else { "+" };
        out.push_str(&format!(
            "| {} | {} ms | {} ms | {sign}{} ms | {sign}{:.1}% |\n",
            r.id, r.base_ms, r.new_ms, r.delta_ms, r.pct
        ));
    }
    out
}

fn run_scenario(root: &Path, s: &Scenario, repeat: u32) -> BenchRecord {
    let runs = repeat.max(1);
    let mut best: Option<u128> = None;
    let mut all_ok = true;
    for _ in 0..runs {
        let p = root.join(&s.touch);
        if let Ok(f) = std::fs::File::open(&p) {
            let _ = f.set_modified(std::time::SystemTime::now());
        }
        let start = Instant::now();
        let status = Command::new(crate::cargo_bin())
            .current_dir(root)
            .arg("check")
            .args(&s.args)
            .status();
        let wall_ms = start.elapsed().as_millis();
        let ok = matches!(status, Ok(st) if st.success());
        all_ok &= ok;
        best = Some(best.map_or(wall_ms, |b| b.min(wall_ms)));
    }
    BenchRecord {
        id: s.id.clone(),
        ok: all_ok,
        wall_ms: best.unwrap_or(0),
    }
}

fn load_scenarios(root: &Path) -> Result<ScenarioFile> {
    let p = root.join("contracts/ci/build-bench-scenarios.v1.json");
    let s =
        std::fs::read_to_string(&p).with_context(|| format!("read scenarios {}", p.display()))?;
    serde_json::from_str(&s).with_context(|| "parse build-bench-scenarios.v1.json")
}

pub fn run_build_bench(
    root: &Path,
    label: Option<String>,
    write: Option<String>,
    compare: Option<String>,
    repeat: u32,
    ingest: bool,
) -> Result<()> {
    let sf = load_scenarios(root)?;
    let label = label.unwrap_or_else(|| "adhoc".to_string());
    eprintln!(
        "build-bench: running {} scenario(s) [{}] × {} (min) …",
        sf.scenarios.len(),
        label,
        repeat.max(1)
    );
    let mut records = Vec::new();
    for s in &sf.scenarios {
        let r = run_scenario(root, s, repeat);
        eprintln!(
            "  {:<36} {}  {} ms",
            r.id,
            if r.ok { "ok" } else { "FAIL" },
            r.wall_ms
        );
        records.push(r);
    }
    let snap = Snapshot {
        schema_version: 1,
        label: label.clone(),
        records,
    };

    if let Some(out) = &write {
        let json = serde_json::to_string_pretty(&snap)? + "\n";
        if let Some(parent) = Path::new(out).parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(out, json).with_context(|| format!("write snapshot {out}"))?;
        eprintln!("build-bench: wrote snapshot {out}");
    }

    if let Some(base_path) = &compare {
        let base_str = std::fs::read_to_string(base_path)
            .with_context(|| format!("read baseline {base_path}"))?;
        let base: Snapshot = serde_json::from_str(&base_str)
            .with_context(|| format!("parse baseline {base_path}"))?;
        let rows = compute_deltas(&base, &snap);
        let md = format_delta_markdown(&label, &rows);
        print!("{md}");
        let report_dir = root.join("graphify-out/build-bench");
        std::fs::create_dir_all(&report_dir).ok();
        let report = report_dir.join("REPORT.md");
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&report)
        {
            let _ = write!(f, "\n{md}");
        }
        let snap_json = serde_json::to_string_pretty(&snap)? + "\n";
        let _ = std::fs::write(report_dir.join(format!("{label}.json")), snap_json);
    }

    if ingest {
        let history_path = root.join("contracts/ci/build-timings-history.v1.jsonl");
        if let Some(html_path) = find_newest_timings_html(root) {
            match std::fs::read_to_string(&html_path) {
                Ok(html) => {
                    let entries = extract_unit_data(&html);
                    eprintln!(
                        "build-bench --ingest: {} non-cached units from {}",
                        entries.len(),
                        html_path.display()
                    );
                    if let Err(e) = append_to_history(&history_path, &label, &entries) {
                        eprintln!("build-bench --ingest: WARN failed to write history: {e}");
                    }
                }
                Err(e) => eprintln!("build-bench --ingest: WARN could not read HTML: {e}"),
            }
        } else {
            eprintln!(
                "build-bench --ingest: no cargo-timing HTML found under target/cargo-timings/"
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(label: &str, recs: &[(&str, bool, u128)]) -> Snapshot {
        Snapshot {
            schema_version: 1,
            label: label.into(),
            records: recs
                .iter()
                .map(|(id, ok, ms)| BenchRecord {
                    id: (*id).into(),
                    ok: *ok,
                    wall_ms: *ms,
                })
                .collect(),
        }
    }

    #[test]
    fn delta_is_new_minus_base_with_pct() {
        let base = snap("baseline", &[("check_vox_db", true, 1000)]);
        let new = snap("phase2", &[("check_vox_db", true, 600)]);
        let d = compute_deltas(&base, &new);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].delta_ms, -400);
        assert!((d[0].pct - (-40.0)).abs() < 0.001);
    }

    #[test]
    fn failed_or_missing_scenarios_are_skipped() {
        let base = snap("b", &[("a", true, 100), ("b", true, 100)]);
        let new = snap("n", &[("a", false, 50), ("c", true, 50)]);
        let d = compute_deltas(&base, &new);
        assert!(d.is_empty(), "no comparable ok-in-both pairs");
    }

    #[test]
    fn markdown_marks_improvement_without_plus_sign() {
        let rows = vec![DeltaRow {
            id: "check_vox_db".into(),
            base_ms: 1000,
            new_ms: 600,
            delta_ms: -400,
            pct: -40.0,
        }];
        let md = format_delta_markdown("Phase 2", &rows);
        assert!(md.contains("-400 ms"));
        assert!(md.contains("-40.0%"));
        assert!(!md.contains("+-400"), "improvement must not get a + prefix");
    }

    #[test]
    fn extract_unit_data_parses_real_cargo_format() {
        // Mirrors the real cargo --timings emission: `const`, float-seconds
        // durations, and `mode:"todo"` on rows that DID record a build time.
        // A zero-duration row (cached / no rebuild) must be dropped regardless
        // of mode — we filter on duration, NOT mode.
        let html = r#"<html><body><script>
const UNIT_DATA = [
  {"i":1,"name":"vox-config","version":"0.1.0","mode":"todo","target":"lib","duration":9.37,"start":0.0},
  {"i":2,"name":"vox-cli","version":"0.1.0","mode":"todo","target":"lib","duration":0.0,"start":0.0},
  {"i":3,"name":"vox-db","version":"0.1.0","mode":"run-custom-build","target":" build-script","duration":2.5,"start":0.0}
];
</script></body></html>"#;
        let records = extract_unit_data(html);
        assert_eq!(
            records.len(),
            2,
            "zero-duration (cached) entries must be excluded"
        );
        let config = records.iter().find(|r| r.name == "vox-config").unwrap();
        assert_eq!(config.duration_ms, 9370, "9.37s -> 9370ms");
        let db = records.iter().find(|r| r.name == "vox-db").unwrap();
        assert_eq!(db.duration_ms, 2500, "2.5s -> 2500ms");
    }

    #[test]
    fn extract_unit_data_returns_empty_when_no_marker() {
        assert!(extract_unit_data("<html>no data here</html>").is_empty());
    }

    #[test]
    fn find_newest_timings_html_returns_none_when_dir_missing() {
        let result = find_newest_timings_html(std::path::Path::new("/nonexistent/path/xyz123"));
        assert!(result.is_none());
    }

    #[test]
    fn append_to_history_creates_and_appends() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("history.jsonl");
        let entries = vec![TimingEntry {
            name: "vox-config 0.1.0".into(),
            duration_ms: 100,
        }];
        append_to_history(&path, "ci-run-1", &entries).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("vox-config"));
        assert!(content.contains("ci-run-1"));
        append_to_history(&path, "ci-run-2", &entries).unwrap();
        let content2 = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content2.lines().count(), 2, "two appends = two JSONL lines");
    }
}
