//! CR-P2 "99.9% uptime over 7 days" measurement.
//!
//! Per honest plan §5.3 and v1-release-criteria CR-P2: "Marquee fleet
//! must hold 99.9% uptime across a rolling 7-day window."
//!
//! Two modes:
//!
//!   1. **Probe-and-append (default):** runs once, probes each app's
//!      /health, appends `{ts, app, live, http_status, error}` rows to
//!      a JSONL monitor log at
//!      `contracts/reports/perf/cr-p2/monitor.jsonl`. Designed to be
//!      called on a cron (every minute / every 5 minutes) so the log
//!      accumulates over time.
//!   2. **Compute-only (`--compute-only`):** doesn't probe; just walks
//!      the existing JSONL log and emits the rolling-7-day uptime
//!      artifact. Useful for `vox audit --gate all` invocations where
//!      we don't want to side-effect by probing.
//!
//! Both modes write the artifact at
//! `contracts/reports/perf/cr-p2/<UTC>-7day.json` with per-app uptime
//! % over the trailing 7-day window. Threshold met iff every "real"
//! app's 7-day uptime ≥ 99.9%.
//!
//! Honest first-run behavior: with an empty monitor log, the artifact
//! reports `samples_in_window: 0` and `met: false` (no evidence of
//! sustained uptime). Publishable per the "first numbers are below
//! bar" rule — and the monitor log is now wired to start accruing
//! real evidence on the next cron tick.

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{BufRead, BufReader, Write};

const MONITOR_LOG_RELPATH: &str = "contracts/reports/perf/cr-p2/monitor.jsonl";
const WINDOW_DAYS: i64 = 7;
const UPTIME_TARGET: f64 = 0.999;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ProbeRow {
    ts: String, // RFC3339 UTC
    app: String,
    live: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    http_status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    apps: Vec<AppEntry>,
}
#[derive(Debug, Deserialize)]
struct AppEntry {
    id: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    live_url: Option<String>,
}

fn main() {
    let compute_only = std::env::args().any(|a| a == "--compute-only");

    let workspace = vox_audit::workspace_root();
    let log_path = workspace.join(MONITOR_LOG_RELPATH);
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).expect("create cr-p2 dir");
    }

    if !compute_only {
        // Probe mode: load manifest, probe each real app, append rows.
        let manifest_path = workspace
            .join("contracts")
            .join("marquee")
            .join("manifest.v1.yaml");
        if !manifest_path.is_file() {
            eprintln!("CR-P2: manifest not found at {}", manifest_path.display());
            std::process::exit(2);
        }
        let body = std::fs::read_to_string(&manifest_path).expect("read manifest");
        let manifest: Manifest = serde_yaml::from_str(&body).expect("parse manifest");
        let now = chrono::Utc::now().to_rfc3339();
        let mut new_rows: Vec<ProbeRow> = Vec::new();
        for app in &manifest.apps {
            if !matches!(app.status.as_str(), "real" | "production") {
                continue;
            }
            let url = resolve_probe_url(app);
            let (live, http_status, error) = probe_health(&url);
            new_rows.push(ProbeRow {
                ts: now.clone(),
                app: app.id.clone(),
                live,
                http_status,
                error,
            });
        }
        append_rows(&log_path, &new_rows);
        eprintln!(
            "CR-P2: appended {} probe row(s) to {}",
            new_rows.len(),
            log_path.display()
        );
    }

    // Compute uptime from the log (whether or not we just probed).
    let rows = load_rows(&log_path);
    let window_start = chrono::Utc::now() - chrono::Duration::days(WINDOW_DAYS);
    let in_window: Vec<&ProbeRow> = rows
        .iter()
        .filter(|r| {
            chrono::DateTime::parse_from_rfc3339(&r.ts)
                .map(|t| t.to_utc() >= window_start)
                .unwrap_or(false)
        })
        .collect();

    // Per-app rollup.
    let mut by_app: std::collections::BTreeMap<String, (u64, u64)> =
        std::collections::BTreeMap::new();
    for r in &in_window {
        let entry = by_app.entry(r.app.clone()).or_insert((0, 0));
        entry.0 += 1;
        if r.live {
            entry.1 += 1;
        }
    }
    let per_app: Vec<serde_json::Value> = by_app
        .iter()
        .map(|(app, (total, live))| {
            let pct = if *total == 0 {
                0.0
            } else {
                *live as f64 / *total as f64
            };
            json!({
                "app": app,
                "samples_in_window": total,
                "live_samples": live,
                "uptime_pct": pct,
                "met_per_app": pct >= UPTIME_TARGET,
            })
        })
        .collect();
    let total_samples: u64 = by_app.values().map(|(t, _)| *t).sum();
    let total_live: u64 = by_app.values().map(|(_, l)| *l).sum();
    let overall_pct = if total_samples == 0 {
        0.0
    } else {
        total_live as f64 / total_samples as f64
    };
    let met = !by_app.is_empty()
        && by_app
            .values()
            .all(|(t, l)| *t > 0 && (*l as f64 / *t as f64) >= UPTIME_TARGET);

    eprintln!(
        "CR-P2: rolling {WINDOW_DAYS}-day uptime = {:.4}% over {total_samples} sample(s) across {} app(s); target {:.2}%",
        100.0 * overall_pct,
        by_app.len(),
        100.0 * UPTIME_TARGET
    );

    let artifact = json!({
        "schema_version": 1,
        "criterion": "CR-P2",
        "measured_at": chrono::Utc::now().to_rfc3339(),
        "monitor_log_path": log_path.display().to_string(),
        "window_days": WINDOW_DAYS,
        "uptime_target": UPTIME_TARGET,
        "per_app": per_app,
        "results": {
            "total_samples_in_window": total_samples,
            "total_live_samples": total_live,
            "overall_uptime_pct": overall_pct,
        },
        "threshold": {
            "target_per_app_uptime": UPTIME_TARGET,
            "met": met,
        },
        "measurement_notes": [
            format!("Log shape: JSONL rows {{ts, app, live, http_status?, error?}} appended on each probe-mode invocation."),
            format!("Run `cargo run -p vox-audit --bin cr-p2` on a cron (every 1-5 min) to accumulate evidence."),
            format!("`--compute-only` skips the probe step and just rolls up the existing log."),
            format!("First-run sample count will be tiny; published evidence accrues over time."),
        ]
    });
    let body = serde_json::to_string_pretty(&artifact).expect("serialize");
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let out_path = log_path.parent().unwrap().join(format!("{date}-7day.json"));
    std::fs::write(&out_path, body).expect("write artifact");
    eprintln!("artifact: {}", out_path.display());

    if !met {
        std::process::exit(1);
    }
}

fn resolve_probe_url(app: &AppEntry) -> String {
    if let Some(url) = &app.live_url {
        return url.clone();
    }
    let env_key = format!(
        "VOX_CR_P1_{}_URL",
        app.id.to_ascii_uppercase().replace('-', "_")
    );
    if let Ok(url) = std::env::var(&env_key) {
        return url;
    }
    let port = match app.id.as_str() {
        "marquee-app" => 8080,
        "marquee-todo-auth" => 8081,
        "marquee-chat" => 8082,
        _ => 8000,
    };
    format!("http://127.0.0.1:{port}/health")
}

fn probe_health(url: &str) -> (bool, Option<u16>, Option<String>) {
    use std::io::Read;
    use std::net::TcpStream;

    let (host, port, path) = match parse_http_url(url) {
        Some(t) => t,
        None => return (false, None, Some(format!("malformed url: {url}"))),
    };
    let addr = format!("{host}:{port}");
    let socket_addr = match addr.parse() {
        Ok(a) => a,
        Err(_) => {
            let mut addrs = match std::net::ToSocketAddrs::to_socket_addrs(&addr) {
                Ok(a) => a,
                Err(e) => return (false, None, Some(format!("resolve {addr}: {e}"))),
            };
            match addrs.next() {
                Some(a) => a,
                None => return (false, None, Some(format!("resolve {addr}: no addrs"))),
            }
        }
    };
    let mut stream = match TcpStream::connect_timeout(&socket_addr, vox_config::timeouts::D_5S) {
        Ok(s) => s,
        Err(e) => return (false, None, Some(format!("connect: {e}"))),
    };
    let _ = stream.set_read_timeout(Some(vox_config::timeouts::D_5S));
    let _ = stream.set_write_timeout(Some(vox_config::timeouts::D_5S));
    let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    if let Err(e) = stream.write_all(request.as_bytes()) {
        return (false, None, Some(format!("write: {e}")));
    }
    let mut buf = String::new();
    if let Err(e) = stream.take(8192).read_to_string(&mut buf) {
        return (false, None, Some(format!("read: {e}")));
    }
    let first_line = buf.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.splitn(3, ' ').collect();
    let status: Option<u16> = parts.get(1).and_then(|s| s.parse().ok());
    let live = matches!(status, Some(s) if (200..300).contains(&s));
    (live, status, None)
}

fn parse_http_url(url: &str) -> Option<(String, u16, String)> {
    let rest = url.strip_prefix("http://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rfind(':') {
        Some(i) => {
            let host = authority[..i].to_string();
            let port: u16 = authority[i + 1..].parse().ok()?;
            (host, port)
        }
        None => (authority.to_string(), 80),
    };
    Some((host, port, path.to_string()))
}

fn append_rows(log_path: &std::path::Path, rows: &[ProbeRow]) {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .expect("open monitor log");
    for r in rows {
        let line = serde_json::to_string(r).expect("serialize row");
        writeln!(file, "{line}").expect("write row");
    }
}

fn load_rows(log_path: &std::path::Path) -> Vec<ProbeRow> {
    let Ok(file) = std::fs::File::open(log_path) else {
        return Vec::new();
    };
    let reader = BufReader::new(file);
    reader
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_then_load_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let log = tmp.path().join("m.jsonl");
        let rows = vec![
            ProbeRow {
                ts: "2026-05-23T00:00:00+00:00".into(),
                app: "marquee-app".into(),
                live: true,
                http_status: Some(200),
                error: None,
            },
            ProbeRow {
                ts: "2026-05-23T00:01:00+00:00".into(),
                app: "marquee-app".into(),
                live: false,
                http_status: None,
                error: Some("connect refused".into()),
            },
        ];
        append_rows(&log, &rows);
        let loaded = load_rows(&log);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].app, "marquee-app");
        assert!(loaded[0].live);
        assert!(!loaded[1].live);
        assert_eq!(loaded[1].error.as_deref(), Some("connect refused"));
    }

    #[test]
    fn load_returns_empty_for_missing_log() {
        let tmp = tempfile::tempdir().unwrap();
        let log = tmp.path().join("missing.jsonl");
        let rows = load_rows(&log);
        assert!(rows.is_empty());
    }

    #[test]
    fn parse_http_url_handles_localhost_with_port() {
        let (h, p, path) = parse_http_url("http://127.0.0.1:8080/health").unwrap();
        assert_eq!(h, "127.0.0.1");
        assert_eq!(p, 8080);
        assert_eq!(path, "/health");
    }
}
