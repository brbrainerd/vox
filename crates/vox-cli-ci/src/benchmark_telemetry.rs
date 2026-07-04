//! Opt-in unified benchmark telemetry to Codex (`research_metrics` type `benchmark_event`).
//!
//! ## Env precedence
//!
//! - **`VOX_BENCHMARK_TELEMETRY`**: when `1` or `true`, benchmark rows are written.
//! - **`VOX_SYNTAX_K_TELEMETRY`**: when set, gates syntax-k rows; **if unset**, falls back to
//!   `VOX_BENCHMARK_TELEMETRY` (see [`record_syntax_k_opt`]).
//!
//! SSOT: `docs/src/reference/env-vars.md`, trust boundaries `docs/src/architecture/telemetry-trust-ssot.md`.
//!
//! Uses [`DbConfig::resolve_canonical`] and [`vox_repository::discover_repository_or_fallback`] for
//! connection + `repository_id`. When **`VOX_REPOSITORY_ROOT`** is set to a non-empty path, discovery
//! starts there instead of [`std::env::current_dir`] so CLI subprocesses can match MCP
//! [`vox_repository::RepositoryContext`].

use std::sync::OnceLock;

use vox_db::{DbConfig, VoxDb};

fn telemetry_enabled() -> bool {
    if !vox_telemetry::is_master_enabled() {
        return false;
    }
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxBenchmarkTelemetry)
        .expose()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn syntax_k_telemetry_enabled() -> bool {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSyntaxKTelemetry)
        .expose()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or_else(telemetry_enabled)
}

fn telemetry_discovery_start() -> std::path::PathBuf {
    if let Some(p) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxRepositoryRoot).expose()
    {
        let p = p.trim();
        if !p.is_empty() {
            return std::path::PathBuf::from(p);
        }
    }
    match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!(
                target: "vox.benchmark_telemetry",
                error = %e,
                "current_dir failed; using empty path for repository discovery"
            );
            std::path::PathBuf::new()
        }
    }
}

fn repository_id_for_telemetry() -> String {
    let start = telemetry_discovery_start();
    vox_repository::discover_repository_or_fallback(&start).repository_id
}

fn telemetry_runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("benchmark telemetry tokio runtime")
    })
}

/// Async: connect and write one benchmark row (no-op unless `VOX_BENCHMARK_TELEMETRY` is set).
pub async fn record_opt(name: &str, metric_value: Option<f64>, details: Option<serde_json::Value>) {
    record_opt_with_unit(name, metric_value, None, details).await;
}

/// Like [`record_opt`] but forwards `metric_value_unit` (e.g. `"seconds"`) to Codex.
pub async fn record_opt_with_unit(
    name: &str,
    metric_value: Option<f64>,
    metric_value_unit: Option<&str>,
    details: Option<serde_json::Value>,
) {
    if !telemetry_enabled() {
        return;
    }
    let Ok(cfg) = DbConfig::resolve_canonical() else {
        tracing::debug!(target: "vox.benchmark_telemetry", "skip: db config unresolved");
        return;
    };
    let rid = repository_id_for_telemetry();
    let details_bytes = details
        .as_ref()
        .and_then(|d| serde_json::to_vec(d).ok())
        .map(|v| v.len())
        .unwrap_or(0usize);
    tracing::debug!(
        target: "vox.benchmark_telemetry",
        metric_name = name,
        metric_value = metric_value,
        metric_value_unit = metric_value_unit.unwrap_or(""),
        details_bytes,
        "record_opt_with_unit attempt"
    );
    match VoxDb::connect(cfg).await {
        Ok(db) => {
            if let Err(e) = db
                .record_benchmark_event(&rid, name, metric_value, metric_value_unit, details)
                .await
            {
                tracing::debug!(
                    target: "vox.benchmark_telemetry",
                    error = %e,
                    "record_benchmark_event failed"
                );
            }
        }
        Err(e) => tracing::debug!(
            target: "vox.benchmark_telemetry",
            error = %e,
            "VoxDb::connect failed"
        ),
    }
}

/// Async: connect and write one syntax-k telemetry row (no-op unless telemetry env is enabled).
pub async fn record_syntax_k_opt(
    name: &str,
    fixture_id: &str,
    metric_value: Option<f64>,
    details: Option<serde_json::Value>,
) {
    if !syntax_k_telemetry_enabled() {
        return;
    }
    let Ok(cfg) = DbConfig::resolve_canonical() else {
        tracing::debug!(target: "vox.benchmark_telemetry", "skip syntax-k: db config unresolved");
        return;
    };
    let rid = repository_id_for_telemetry();
    match VoxDb::connect(cfg).await {
        Ok(db) => {
            if let Err(e) = db
                .record_syntax_k_event(&rid, name, fixture_id, metric_value, details)
                .await
            {
                tracing::debug!(
                    target: "vox.benchmark_telemetry",
                    error = %e,
                    "record_syntax_k_event failed"
                );
            }
        }
        Err(e) => tracing::debug!(
            target: "vox.benchmark_telemetry",
            error = %e,
            "VoxDb::connect failed for syntax-k"
        ),
    }
}

/// Blocking variant for sync CLI paths (no Tokio handle required).
pub fn record_opt_blocking(
    name: &str,
    metric_value: Option<f64>,
    details: Option<serde_json::Value>,
) {
    record_opt_with_unit_blocking(name, metric_value, None, details);
}

/// Blocking variant for sync CLI paths that includes a metric unit.
pub fn record_opt_with_unit_blocking(
    name: &str,
    metric_value: Option<f64>,
    metric_value_unit: Option<&str>,
    details: Option<serde_json::Value>,
) {
    if !telemetry_enabled() {
        return;
    }
    let Ok(cfg) = DbConfig::resolve_canonical() else {
        tracing::debug!(target: "vox.benchmark_telemetry", "skip: db config unresolved");
        return;
    };
    let rid = repository_id_for_telemetry();
    let details_bytes = details
        .as_ref()
        .and_then(|d| serde_json::to_vec(d).ok())
        .map(|v| v.len())
        .unwrap_or(0usize);
    tracing::debug!(
        target: "vox.benchmark_telemetry",
        metric_name = name,
        metric_value = metric_value,
        metric_value_unit = metric_value_unit.unwrap_or(""),
        details_bytes,
        "record_opt_with_unit_blocking attempt"
    );
    telemetry_runtime().block_on(async {
        match VoxDb::connect(cfg).await {
            Ok(db) => {
                if let Err(e) = db
                    .record_benchmark_event(&rid, name, metric_value, metric_value_unit, details)
                    .await
                {
                    tracing::debug!(
                        target: "vox.benchmark_telemetry",
                        error = %e,
                        "record_benchmark_event failed"
                    );
                }
            }
            Err(e) => tracing::debug!(
                target: "vox.benchmark_telemetry",
                error = %e,
                "VoxDb::connect failed"
            ),
        }
    });
}

/// Blocking variant for sync syntax-k telemetry paths.
pub fn record_syntax_k_opt_blocking(
    name: &str,
    fixture_id: &str,
    metric_value: Option<f64>,
    details: Option<serde_json::Value>,
) {
    if !syntax_k_telemetry_enabled() {
        return;
    }
    let Ok(cfg) = DbConfig::resolve_canonical() else {
        tracing::debug!(target: "vox.benchmark_telemetry", "skip syntax-k: db config unresolved");
        return;
    };
    let rid = repository_id_for_telemetry();
    telemetry_runtime().block_on(async {
        match VoxDb::connect(cfg).await {
            Ok(db) => {
                if let Err(e) = db
                    .record_syntax_k_event(&rid, name, fixture_id, metric_value, details)
                    .await
                {
                    tracing::debug!(
                        target: "vox.benchmark_telemetry",
                        error = %e,
                        "record_syntax_k_event failed"
                    );
                }
            }
            Err(e) => tracing::debug!(
                target: "vox.benchmark_telemetry",
                error = %e,
                "VoxDb::connect failed for syntax-k"
            ),
        }
    });
}
