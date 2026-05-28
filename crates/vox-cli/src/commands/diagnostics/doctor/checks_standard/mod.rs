//! Default `vox doctor` checks: optional test-health tools, then full toolchain audit.

mod compile_target;
mod gpu_hardware;
mod llm_routing;
mod model_catalog;
mod model_telemetry;
mod secrets;
mod tail;
mod test_health;
mod toolchain;
mod vox_ignore;
mod web_frontend;

use super::common::Check;

pub async fn run_checks(
    auto_heal: bool,
    test_health: bool,
    compile_target: Option<&str>,
    checks: &mut Vec<Check>,
) {
    if let Some(t) = compile_target.filter(|s| !s.is_empty()) {
        compile_target::run(t, checks);
    }
    if test_health::run(test_health, checks).await {
        return;
    }
    toolchain::run(auto_heal, checks).await;
    secrets::run(auto_heal, checks).await;
    llm_routing::run(checks);
    gpu_hardware::run(checks).await;
    vox_ignore::run(auto_heal, checks).await;
    web_frontend::run(checks).await;
    model_telemetry::run(checks).await;
    model_catalog::run(checks).await;
    tail::run(auto_heal, checks).await;
}
