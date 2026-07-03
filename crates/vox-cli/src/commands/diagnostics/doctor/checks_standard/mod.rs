//! Default `vox doctor` checks: optional test-health tools, then full toolchain audit.

mod binary_ssot;
mod build_health;
mod compile_target;
mod freshness;
mod gpu_hardware;
mod llm_routing;
mod model_catalog;
mod model_telemetry;
mod secrets;
mod tail;
mod test_health;
pub mod tier_deps;
mod toolchain;
mod vox_ignore;
mod web_frontend;

use super::common::Check;

pub(crate) use build_health::parse_diag_id;

/// Stable diag-id registry, for `--diag` unknown-id error messages.
pub(crate) fn known_diag_ids() -> &'static [&'static str] {
    build_health::KNOWN_DIAGNOSIS_IDS
}

/// Run only the check-set covering `id`. Returns `false` for unregistered ids.
pub(crate) async fn run_diag_check(id: &str, checks: &mut Vec<Check>) -> bool {
    let Some(kind) = build_health::check_kind_for_diag(id) else {
        return false;
    };
    build_health::run_check_for_diag(kind, checks).await;
    true
}

pub async fn run_checks(
    auto_heal: bool,
    test_health: bool,
    compile_target: Option<&str>,
    tier: &str,
    checks: &mut Vec<Check>,
) {
    if let Some(t) = compile_target.filter(|s| !s.is_empty()) {
        compile_target::run(t, checks);
    }
    if test_health::run(test_health, checks).await {
        return;
    }
    toolchain::run(auto_heal, checks).await;
    build_health::run(auto_heal, checks).await;
    freshness::run(checks);
    binary_ssot::run(checks);
    secrets::run(auto_heal, checks).await;
    llm_routing::run(checks);
    gpu_hardware::run(checks).await;
    vox_ignore::run(auto_heal, checks).await;
    web_frontend::run(checks).await;
    model_telemetry::run(checks).await;
    model_catalog::run(checks).await;
    tail::run(auto_heal, checks).await;

    // Per-tier runtime-optional dep surfacing (reads distribution SSOT).
    let dep_statuses = tier_deps::check_runtime_optional_deps(tier);
    for s in dep_statuses {
        checks.push(Check::new(
            format!("tier dep: {}", s.name),
            s.present,
            if s.present {
                format!("{} — found", s.name)
            } else {
                s.hint
            },
        ));
    }
}
