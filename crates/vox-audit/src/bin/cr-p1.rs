//! CR-P1 "three apps deployed live" measurement.
//!
//! Per `docs/superpowers/specs/2026-05-21-v1-honest-completion-plan.md` §5.2
//! and v1-release-criteria CR-P1: "Three marquee apps must be deployed
//! live and responding on health endpoints."
//!
//! What this v1.0 sweep does:
//!
//!   1. Loads `contracts/marquee/manifest.v1.yaml` to discover each
//!      marquee app's expected live URL. URLs come from either:
//!        - an explicit `live_url:` field in the manifest entry
//!        - the env var `VOX_CR_P1_<APP_ID>_URL` (uppercase, dashes→_)
//!        - falls back to `http://127.0.0.1:<port>/health` per slot
//!   2. For each "real" app, HTTP-GET the /health endpoint with a 5-second
//!      timeout. 2xx = live, anything else = not live.
//!   3. Writes `contracts/reports/perf/cr-p1/<UTC>.json` with per-app
//!      live status. Threshold met iff every "real" app is live AND
//!      there are at least 3 of them.
//!
//! Local-dev workflow: bring up your apps (e.g. `pnpm dev` in each, or
//! `docker compose up`) so they listen on the configured ports, then
//! re-run. Cold-start / no-apps-running is a real `met: false` artifact
//! per honest plan §3.x — the sub-bar number is publishable evidence.
//!
//! Out of scope (deferred to v1.x):
//!   - End-to-end "vox build → docker build → docker run" automation
//!     for the marquees that aren't web-app-shaped yet (slots 2-3).
//!     Slot 1 (marquee_app) is a Vite/React app with a Dockerfile;
//!     slots 2-3 are Vox source fixtures that need codegen-to-deployable
//!     wiring (their own follow-on track).
//!   - Cross-host probing: this binary assumes localhost. CI / mesh
//!     deployments override per-app URLs via the env-var convention.

use serde::Deserialize;
use serde_json::json;

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
    fixture_path: String,
    /// Optional explicit live URL — when set, takes priority over the
    /// env-var convention.
    #[serde(default)]
    live_url: Option<String>,
}

#[derive(Debug, serde::Serialize)]
struct AppLiveness {
    id: String,
    status: String,
    fixture_path: String,
    probe_url: String,
    live: bool,
    http_status: Option<u16>,
    response_time_ms: Option<u64>,
    error: Option<String>,
}

/// Default port assignments for the three canonical marquee slots.
/// Used when no explicit `live_url` and no env-var override is set.
fn default_port_for(app_id: &str) -> u16 {
    match app_id {
        "marquee-app" => 8080,
        "marquee-todo-auth" => 8081,
        "marquee-chat" => 8082,
        _ => 8000,
    }
}

fn main() {
    let workspace = vox_audit::workspace_root();
    let manifest_path = workspace
        .join("contracts")
        .join("marquee")
        .join("manifest.v1.yaml");
    if !manifest_path.is_file() {
        eprintln!(
            "CR-P1: manifest not found at {}; cannot run",
            manifest_path.display()
        );
        std::process::exit(2);
    }
    let body = std::fs::read_to_string(&manifest_path).expect("read manifest");
    let manifest: Manifest = serde_yaml::from_str(&body).expect("parse manifest");

    let mut probes: Vec<AppLiveness> = Vec::new();
    for app in &manifest.apps {
        if !matches!(app.status.as_str(), "real" | "production") {
            continue;
        }
        let probe_url = resolve_probe_url(app);
        let (live, http_status, response_time_ms, error) = probe_health(&probe_url);
        probes.push(AppLiveness {
            id: app.id.clone(),
            status: app.status.clone(),
            fixture_path: app.fixture_path.clone(),
            probe_url,
            live,
            http_status,
            response_time_ms,
            error,
        });
    }

    let total = probes.len() as u32;
    let live_count = probes.iter().filter(|p| p.live).count() as u32;
    let live_pct = if total == 0 {
        0.0
    } else {
        f64::from(live_count) / f64::from(total)
    };
    // CR-P1 bar: ≥ 3 real apps AND ALL of them live.
    let met = total >= 3 && live_count == total;

    eprintln!(
        "CR-P1: {live_count}/{total} marquee apps live ({:.1}%)",
        100.0 * live_pct
    );
    for p in &probes {
        let flag = if p.live { "✓" } else { "✗" };
        let detail = match (p.http_status, &p.error) {
            (Some(s), _) => format!("HTTP {}", s),
            (None, Some(e)) => format!("error: {e}"),
            (None, None) => "no response".to_string(),
        };
        eprintln!(
            "  {flag} {id:24}  {url}  →  {detail}",
            id = p.id,
            url = p.probe_url
        );
    }

    let artifact = json!({
        "schema_version": 1,
        "criterion": "CR-P1",
        "measured_at": chrono::Utc::now().to_rfc3339(),
        "manifest_path": manifest_path.display().to_string(),
        "per_app": probes,
        "results": {
            "real_apps_total": total,
            "real_apps_live": live_count,
            "live_pct": live_pct,
        },
        "threshold": {
            "target_real_apps_total": 3,
            "target_all_live": true,
            "met": met,
        },
        "measurement_notes": [
            "live = HTTP GET <probe_url> returned 2xx within 5s.",
            "probe_url resolution: (1) manifest.apps[].live_url if set; (2) env VOX_CR_P1_<APP_ID>_URL (uppercase, dashes→_); (3) http://127.0.0.1:<default_port>/health.",
            "Default ports per slot: marquee-app=8080, marquee-todo-auth=8081, marquee-chat=8082.",
            "Local-dev scope: this measurement assumes the three slots are reachable on the loopback interface; the v1.0 acceptance bar is local-dev reachability, not production OCI hosting. Canonical bring-up runbook: `vox run scripts/start-marquee.vox` (handles slot 2 + slot 3 native binaries) + docker (slot 1's nginx SPA). Slot 1 has no Rust backend at runtime — it ships as a static SPA + nginx — so it follows the docker recipe in apps/interop/marquee_app/Dockerfile rather than the native-binary recipe.",
            "Sub-bar artifacts are publishable evidence per honest plan §3.x — even a 1/3 result documents real measured progress."
        ]
    });

    let body = serde_json::to_string_pretty(&artifact).expect("serialize");
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let out_dir = workspace
        .join("contracts")
        .join("reports")
        .join("perf")
        .join("cr-p1");
    std::fs::create_dir_all(&out_dir).expect("create cr-p1 dir");
    let out_path = out_dir.join(format!("{date}.json"));
    std::fs::write(&out_path, body).expect("write artifact");
    eprintln!("artifact: {}", out_path.display());

    if !met {
        std::process::exit(1);
    }
}

/// Resolve the URL to probe for a given app, per the documented order.
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
    let port = default_port_for(&app.id);
    format!("http://127.0.0.1:{port}/health")
}

/// HTTP-GET the URL with a 5s timeout. Returns (live, http_status,
/// response_time_ms, error). Redirects are not followed: a 3xx answer
/// counts as not-live, matching the "2xx within 5s" bar.
fn probe_health(url: &str) -> (bool, Option<u16>, Option<u64>, Option<String>) {
    let client = match reqwest::blocking::Client::builder()
        .timeout(vox_config::timeouts::D_5S)
        .connect_timeout(vox_config::timeouts::D_5S)
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(c) => c,
        Err(e) => return (false, None, None, Some(format!("client: {e}"))),
    };
    let start = std::time::Instant::now();
    match client.get(url).send() {
        Ok(resp) => {
            let elapsed_ms = start.elapsed().as_millis() as u64;
            let status = resp.status().as_u16();
            (
                resp.status().is_success(),
                Some(status),
                Some(elapsed_ms),
                None,
            )
        }
        Err(e) => (false, None, None, Some(format!("request: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ports_are_distinct_for_three_slots() {
        assert_ne!(
            default_port_for("marquee-app"),
            default_port_for("marquee-todo-auth")
        );
        assert_ne!(
            default_port_for("marquee-todo-auth"),
            default_port_for("marquee-chat")
        );
        assert_ne!(
            default_port_for("marquee-app"),
            default_port_for("marquee-chat")
        );
    }
}
