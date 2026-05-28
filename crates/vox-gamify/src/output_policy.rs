//! CLI / UX output gating (anti-spam, verbosity).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::config_gate;

/// Upper bound on unsolicited Ludus CLI messages per rolling window (default: 12 / hour).
fn max_messages_per_hour() -> u32 {
    std::env::var("VOX_LUDUS_MAX_MESSAGES_PER_HOUR")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(12)
}

#[derive(Default)]
struct BurstState {
    window_start: Option<Instant>,
    count: u32,
}

static BURST: Mutex<BurstState> = Mutex::new(BurstState {
    window_start: None,
    count: 0,
});

/// `quiet` | `normal` | `rich` — default `normal`. Overrides overlays for streak/level lines.
pub fn ludus_verbosity() -> &'static str {
    match std::env::var("VOX_LUDUS_VERBOSITY")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "quiet" | "0" => "quiet",
        "rich" | "2" => "rich",
        _ => "normal",
    }
}

/// Whether celebration-style CLI lines (streak, level-up) may print.
pub fn cli_celebrations_allowed() -> bool {
    if !config_gate::is_enabled() {
        return false;
    }
    if matches!(
        config_gate::ludus_channel(),
        config_gate::LudusChannel::DigestPriority
    ) {
        return false;
    }
    if !config_gate::overlays_enabled() {
        return false;
    }
    !matches!(ludus_verbosity(), "quiet")
}

/// Rate-limit bursty CLI messages; returns `false` if the hourly budget is exhausted.
pub fn claim_cli_message_budget() -> bool {
    let cap = max_messages_per_hour();
    let mut g = BURST.lock().ok();
    let Some(state) = g.as_mut() else {
        return true;
    };
    let now = Instant::now();
    match state.window_start {
        None => {
            state.window_start = Some(now);
            state.count = 1;
            true
        }
        Some(start) if now.duration_since(start) > vox_config::timeouts::D_3600S => {
            state.window_start = Some(now);
            state.count = 1;
            true
        }
        Some(_) if state.count >= cap => false,
        Some(_) => {
            state.count += 1;
            true
        }
    }
}

/// Combine celebration mode + rate limit.
pub fn should_emit_cli_celebration() -> bool {
    cli_celebrations_allowed() && claim_cli_message_budget()
}
