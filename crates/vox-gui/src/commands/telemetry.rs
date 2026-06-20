//! Tauri commands for the "Telemetry" GUI surface — the anonymous, opt-in
//! contribution control.
//!
//! These mirror the canonical CLI flow (`vox telemetry consent grant|deny|status`
//! and `vox telemetry doctor`) so the GUI and CLI drive the SAME persisted
//! consent state in `~/.config/vox/remote-consent`. No account or login is
//! involved: a contribution is identified only by a random per-install UUID
//! ([`vox_telemetry::install_id`]); the salt used to hash session ids is never
//! uploaded.

use serde::Serialize;
use tauri::command;
use vox_telemetry::config::{
    ConsentState, install_id, is_master_enabled, is_remote_allowed, remote_consent,
    set_remote_consent,
};

/// Non-sensitive snapshot of the telemetry consent state for the GUI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryConsentDto {
    /// `"granted" | "denied" | "unset"`. `unset` = first-run, treated as denied.
    pub state: String,
    /// Whether remote upload is actually allowed right now (master on AND granted).
    pub remote_allowed: bool,
    /// Master switch (org policy / `VOX_TELEMETRY` / user config). When false,
    /// nothing is collected or uploaded regardless of consent.
    pub master_enabled: bool,
    /// Anonymous per-install identifier (random UUID). This is the ONLY identity
    /// attached to a contribution — there is no account.
    pub install_id: String,
}

fn state_str(state: ConsentState) -> &'static str {
    match state {
        ConsentState::Granted => "granted",
        ConsentState::Denied => "denied",
        ConsentState::Unset => "unset",
    }
}

fn snapshot() -> TelemetryConsentDto {
    TelemetryConsentDto {
        state: state_str(remote_consent()).to_string(),
        remote_allowed: is_remote_allowed(),
        master_enabled: is_master_enabled(),
        install_id: install_id(),
    }
}

/// Report current remote-consent state + the anonymous install id.
#[command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over vox_telemetry consent; logic covered by vox-telemetry tests
pub fn get_telemetry_consent() -> TelemetryConsentDto {
    snapshot()
}

/// Opt in (`grant = true`) or out (`grant = false`) of anonymous remote
/// contribution. Persists to `~/.config/vox/remote-consent` (same file the CLI
/// uses) and returns the refreshed snapshot.
#[command]
// toestub-ignore(skeleton/untested-pub-api) — thin Tauri IPC over vox_telemetry::set_remote_consent; logic covered by vox-telemetry tests
pub fn set_telemetry_consent(grant: bool) -> TelemetryConsentDto {
    set_remote_consent(if grant {
        ConsentState::Granted
    } else {
        ConsentState::Denied
    });
    snapshot()
}
