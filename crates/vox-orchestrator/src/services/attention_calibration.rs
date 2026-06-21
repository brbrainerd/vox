//! Background calibration: every few minutes, aggregate recent attention events and update the
//! running interruption-calibration config so the ask-threshold adapts to which surfaces the pilot
//! actually engages vs. rejects. Closes the Phase-D learn loop (audit #4) at runtime.

use std::sync::Arc;

use crate::Orchestrator;

/// Minimum events in the ring before we bother recalibrating (avoid acting on cold-start noise).
const MIN_EVENTS_TO_CALIBRATE: usize = 10;

/// Spawn the periodic attention-calibration job. Ticks every 5 minutes.
pub fn spawn_attention_calibration(orch: Arc<Orchestrator>) {
    let mut tick = tokio::time::interval(vox_config::timeouts::D_300S);
    tokio::spawn(async move {
        loop {
            tick.tick().await;
            // Snapshot the recent in-memory event ring (newest 100).
            let events = {
                let bm = crate::sync_lock::rw_read(&orch.budget_manager);
                bm.attention_events_snapshot(100)
            };
            if events.len() < MIN_EVENTS_TO_CALIBRATE {
                continue;
            }
            // Recalibrate from the current base and write it back under the config write lock.
            let base = {
                let cfg = crate::sync_lock::rw_read(&orch.config);
                cfg.interruption_calibration.clone()
            };
            let updated = crate::attention::calibrator::recalibrate(base, &events);
            {
                let mut cfg = crate::sync_lock::rw_write(&orch.config);
                cfg.interruption_calibration = updated;
            }
            tracing::debug!(
                target: "vox_attention_calibration",
                events = events.len(),
                "recalibrated interruption offsets from attention events"
            );
        }
    });
}
