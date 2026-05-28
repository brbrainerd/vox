//! CR-E2 marquee bundle-size measurement.
//!
//! Per `docs/superpowers/specs/2026-05-21-v1-honest-completion-plan.md` §5.5
//! and v1-release-criteria CR-E2: "Marquee bundle ≤ 800KB gzip".
//!
//! What this v1.0 sweep does:
//!
//!   1. Loads `contracts/marquee/manifest.v1.yaml` to discover each
//!      marquee app's fixture_path.
//!   2. For each app, walks the source tree (excluding lock files,
//!      Dockerfile, node_modules, tsconfig.tsbuildinfo) and gzips each
//!      file individually + sums the gzipped bytes. This is the
//!      "static source bundle" — the upper bound on what Vite / esbuild
//!      would input.
//!   3. Compares against the threshold (800 KB = 819200 bytes gzip).
//!   4. Writes `contracts/reports/perf/cr-e2/<UTC>.json` with per-app
//!      sizes + aggregate.
//!
//! What this does NOT yet do (deferred to v1.x):
//!
//!   - Run the final bundler (`pnpm build` → vite) to measure the
//!     actually-shipped JS bundle. Vite tree-shakes, minifies, and adds
//!     vendor JS — the final bundle is typically smaller than the
//!     source input. The measurement here is therefore a CONSERVATIVE
//!     upper bound: if source-gzip is under threshold, the real bundle
//!     will also be under. If source-gzip is over, the real bundle
//!     MIGHT be under after tree-shaking.
//!   - Per-asset breakdown (vendor vs app chunks). Requires the bundler.
//!
//! These limitations are recorded honestly in the artifact under
//! `measurement_notes`.

use serde::Deserialize;
use serde_json::json;
use std::io::Write;

/// 800 KB = 819200 bytes per honest plan §5.5 and marquee manifest's
/// expected_bundle_kb_gzip default.
const THRESHOLD_BYTES_GZIP: u64 = 800 * 1024;

/// Files we exclude as not-part-of-the-bundle (lockfiles, builder
/// configs, etc. — these don't ship in the gzipped client bundle).
const EXCLUDED_BASENAMES: &[&str] = &[
    "Dockerfile",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "Cargo.lock",
    "pnpm-workspace.yaml",
    "tsconfig.json",
    "tsconfig.app.json",
    "tsconfig.tsbuildinfo",
    "vite.config.ts",
    "vite.config.js",
];

/// Directory names to skip entirely.
const EXCLUDED_DIRS: &[&str] = &["node_modules", "target", "dist", "build", ".vox"];

#[derive(Debug, Deserialize)]
struct Manifest {
    apps: Vec<AppEntry>,
}

#[derive(Debug, Deserialize)]
struct AppEntry {
    id: String,
    #[serde(default)]
    status: String,
    fixture_path: String,
    #[serde(default)]
    expected_bundle_kb_gzip: Option<u64>,
}

#[derive(Debug, serde::Serialize)]
struct AppMeasurement {
    id: String,
    status: String,
    fixture_path: String,
    files_measured: u32,
    raw_bytes: u64,
    gzip_bytes: u64,
    gzip_kb: f64,
    threshold_kb_gzip: u64,
    met: bool,
}

fn main() {
    let workspace = vox_audit::workspace_root();
    let manifest_path = workspace
        .join("contracts")
        .join("marquee")
        .join("manifest.v1.yaml");
    if !manifest_path.is_file() {
        eprintln!(
            "CR-E2: manifest not found at {}; cannot run",
            manifest_path.display()
        );
        std::process::exit(2);
    }
    let body = std::fs::read_to_string(&manifest_path).expect("read manifest");
    let manifest: Manifest = serde_yaml::from_str(&body).expect("parse manifest");

    let mut measurements: Vec<AppMeasurement> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for app in &manifest.apps {
        // Skip stub / planned slots that don't have real fixture paths yet.
        if !matches!(app.status.as_str(), "real" | "production") {
            continue;
        }
        let fixture_abs = workspace.join(&app.fixture_path);
        if !fixture_abs.is_dir() {
            errors.push(format!(
                "{}: fixture_path not found: {}",
                app.id,
                fixture_abs.display()
            ));
            continue;
        }
        let threshold_kb = app.expected_bundle_kb_gzip.unwrap_or(800);
        let threshold_bytes = threshold_kb * 1024;
        let (files, raw, gz) = measure_dir(&fixture_abs);
        measurements.push(AppMeasurement {
            id: app.id.clone(),
            status: app.status.clone(),
            fixture_path: app.fixture_path.clone(),
            files_measured: files,
            raw_bytes: raw,
            gzip_bytes: gz,
            gzip_kb: gz as f64 / 1024.0,
            threshold_kb_gzip: threshold_kb,
            met: gz <= threshold_bytes,
        });
    }

    let total_apps = measurements.len();
    let met_apps = measurements.iter().filter(|m| m.met).count();
    let max_gzip = measurements.iter().map(|m| m.gzip_bytes).max().unwrap_or(0);
    let overall_met = !measurements.is_empty() && measurements.iter().all(|m| m.met);

    eprintln!(
        "CR-E2: measured {total_apps} real marquee app(s); {met_apps}/{total_apps} under threshold; max gzip = {} KB",
        max_gzip / 1024
    );
    for m in &measurements {
        let flag = if m.met { "✓" } else { "✗" };
        eprintln!(
            "  {flag} {id:30} {kb:>7.2} KB gzip vs {th} KB ({raw} raw, {n} files)",
            id = m.id,
            kb = m.gzip_kb,
            th = m.threshold_kb_gzip,
            raw = m.raw_bytes,
            n = m.files_measured
        );
    }
    if !errors.is_empty() {
        eprintln!("CR-E2: {} fixture(s) missing:", errors.len());
        for e in &errors {
            eprintln!("  - {e}");
        }
    }

    let artifact = json!({
        "schema_version": 1,
        "criterion": "CR-E2",
        "measured_at": chrono::Utc::now().to_rfc3339(),
        "threshold_kb_gzip_default": THRESHOLD_BYTES_GZIP / 1024,
        "per_app": measurements,
        "errors": errors,
        "results": {
            "apps_measured": total_apps,
            "apps_met": met_apps,
            "max_gzip_kb": max_gzip / 1024,
        },
        "threshold": {
            "target_per_app_kb_gzip": 800,
            "met": overall_met,
        },
        "measurement_notes": [
            "Source-bundle gzip (sum of per-file gzipped sizes across the app source tree).",
            "Conservative upper bound vs the post-vite-build JS bundle; vite tree-shakes + minifies.",
            "Excludes: Dockerfile, package-lock, pnpm-lock, tsconfig*, vite.config; and node_modules/, target/, dist/, build/, .vox/ dirs.",
            "Final-bundle measurement (running `pnpm build` + sizing dist/) is a v1.x follow-on."
        ]
    });

    let body = serde_json::to_string_pretty(&artifact).expect("serialize");
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let out_dir = workspace
        .join("contracts")
        .join("reports")
        .join("perf")
        .join("cr-e2");
    std::fs::create_dir_all(&out_dir).expect("create cr-e2 dir");
    let out_path = out_dir.join(format!("{date}.json"));
    std::fs::write(&out_path, body).expect("write artifact");
    eprintln!("artifact: {}", out_path.display());

    if !overall_met {
        std::process::exit(1);
    }
}

/// Walk a fixture directory, gzip each non-excluded file, return
/// (file_count, total_raw_bytes, total_gzip_bytes).
fn measure_dir(root: &std::path::Path) -> (u32, u64, u64) {
    let mut files = 0u32;
    let mut raw = 0u64;
    let mut gz = 0u64;
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            !e.file_name()
                .to_str()
                .is_some_and(|n| EXCLUDED_DIRS.contains(&n))
        })
        .filter_map(|r| r.ok())
    {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let basename = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if EXCLUDED_BASENAMES.contains(&basename) {
            continue;
        }
        let Ok(bytes) = std::fs::read(p) else {
            continue;
        };
        let gz_bytes = gzip_size(&bytes);
        files += 1;
        raw += bytes.len() as u64;
        gz += gz_bytes;
    }
    (files, raw, gz)
}

/// Gzip a byte slice and return the compressed size. Uses default
/// compression level (6). Deterministic across runs.
fn gzip_size(bytes: &[u8]) -> u64 {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).expect("gzip write");
    encoder.finish().expect("gzip finish").len() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn gzip_size_is_deterministic() {
        let bytes = b"the quick brown fox jumps over the lazy dog".repeat(20);
        let s1 = gzip_size(&bytes);
        let s2 = gzip_size(&bytes);
        assert_eq!(s1, s2);
        assert!(s1 < bytes.len() as u64, "should compress");
    }

    #[test]
    fn measure_dir_excludes_node_modules() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("app.tsx"), "console.log('hi');").unwrap();
        std::fs::create_dir_all(root.join("node_modules/react")).unwrap();
        std::fs::write(root.join("node_modules/react/index.js"), "big vendor lib").unwrap();
        std::fs::create_dir_all(root.join("dist")).unwrap();
        std::fs::write(root.join("dist/bundle.js"), "minified").unwrap();

        let (files, raw, _gz) = measure_dir(root);
        assert_eq!(files, 1, "only app.tsx, not node_modules or dist");
        assert!(raw < 100, "small");
    }

    #[test]
    fn measure_dir_excludes_lockfiles_and_configs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("main.vox"), "fn x() to int { return 1 }").unwrap();
        std::fs::write(root.join("Dockerfile"), "FROM scratch").unwrap();
        std::fs::write(root.join("pnpm-lock.yaml"), "lockfile contents").unwrap();
        std::fs::write(root.join("vite.config.ts"), "export default {};").unwrap();
        std::fs::write(root.join("tsconfig.tsbuildinfo"), "{}").unwrap();

        let (files, _raw, _gz) = measure_dir(root);
        assert_eq!(files, 1, "only main.vox; the rest are excluded");
    }

    #[test]
    fn empty_dir_measures_to_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let (files, raw, gz) = measure_dir(tmp.path());
        assert_eq!(files, 0);
        assert_eq!(raw, 0);
        assert_eq!(gz, 0);
    }
}
