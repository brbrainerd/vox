//! LSP / IDE diagnostic transitions → Ludus events (optional Codex).
//!
//! Emits `diagnostics_clean` when a document goes from having diagnostics to zero
//! (per-file), with cooldown to avoid spam.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use vox_db::Codex;

struct DocState {
    had_issues: bool,
    last_diagnostics_clean_at: Option<Instant>,
}

static DOC_STATE: OnceLock<Mutex<HashMap<String, DocState>>> = OnceLock::new();

const DIAG_CLEAN_COOLDOWN: Duration = vox_config::timeouts::D_120S;

fn doc_state_lock() -> &'static Mutex<HashMap<String, DocState>> {
    DOC_STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// After the LSP published diagnostics for `doc_key` (typically `uri.as_str()`),
/// optionally record a `diagnostics_clean` event when errors+warnings drop to zero
/// after previously having issues.
pub async fn after_diagnostic_publish(
    db: &Codex,
    doc_key: &str,
    error_count: usize,
    warning_count: usize,
) {
    if !crate::config_gate::is_enabled() {
        return;
    }
    let total = error_count + warning_count;
    let now = Instant::now();
    let emit = {
        let Ok(mut map) = doc_state_lock().lock() else {
            return;
        };
        let ent = map.entry(doc_key.to_string()).or_insert(DocState {
            had_issues: false,
            last_diagnostics_clean_at: None,
        });
        let mut fire = false;
        if total == 0 && ent.had_issues {
            let ok = match ent.last_diagnostics_clean_at {
                Some(t) => now.duration_since(t) >= DIAG_CLEAN_COOLDOWN,
                None => true,
            };
            if ok {
                fire = true;
                ent.last_diagnostics_clean_at = Some(now);
            }
        }
        ent.had_issues = total > 0;
        fire
    };
    if emit {
        let uid = crate::db::canonical_user_id();
        let ev = serde_json::json!({
            "type": "diagnostics_clean",
            "source": "vox-lsp",
            "agent_id": 0u64,
        });
        let _ = crate::event_router::route_event(db, &uid, &ev).await;
    }
}

static CLI_CHECK_LAST: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

fn cli_check_lock() -> &'static Mutex<HashMap<String, Instant>> {
    CLI_CHECK_LAST.get_or_init(|| Mutex::new(HashMap::new()))
}

/// After `vox check` succeeds (zero errors), emit `diagnostics_clean` with per-file cooldown.
pub async fn after_cli_check_clean(db: &Codex, dedupe_key: &str) {
    if !crate::config_gate::is_enabled() {
        return;
    }
    let now = Instant::now();
    {
        let Ok(mut map) = cli_check_lock().lock() else {
            return;
        };
        if let Some(t) = map.get(dedupe_key) {
            if now.duration_since(*t) < DIAG_CLEAN_COOLDOWN {
                return;
            }
        }
        map.insert(dedupe_key.to_string(), now);
    }
    let uid = crate::db::canonical_user_id();
    let ev = serde_json::json!({
        "type": "diagnostics_clean",
        "source": "vox-check",
        "agent_id": 0u64,
    });
    let _ = crate::event_router::route_event(db, &uid, &ev).await;
}
