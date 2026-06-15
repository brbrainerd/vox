/**
 * Central timing, limit, and debounce constants for the Vox GUI.
 * Import these instead of inline numeric literals in setInterval/setTimeout/debounce.
 */

/** Orchestrator status polling fallback when Tauri event stream unavailable (ms). */
export const ORCH_POLL_FALLBACK_MS = 2000;

/** Policy badge refresh interval in sidebar (ms). */
export const POLICY_BADGE_POLL_MS = 60_000;

/** Approvals surface poll interval (ms). */
export const APPROVALS_POLL_MS = 2000;

/** Runs surface poll interval (ms). */
export const RUNS_POLL_MS = 10_000;

/** Gamify surface poll interval (ms). */
export const GAMIFY_POLL_MS = 15_000;

/** Mesh view refresh interval (ms). */
export const MESH_REFRESH_MS = 5000;

/** Matrix routing intentions poll interval (ms). */
export const MATRIX_POLL_MS = 8000;

/** Max dashboard activity stream items retained. */
export const STREAM_CAP = 100;

/** Loquela composer input history cap. */
export const COMPOSER_HISTORY_CAP = 30;

/** Unified search debounce (ms). */
export const SEARCH_DEBOUNCE_MS = 200;

/** Memory auto-recall debounce (ms). */
export const MEMORY_RECALL_DEBOUNCE_MS = 450;

/** Command palette backend preview hit limit. */
export const PALETTE_PREVIEW_LIMIT = 8;

/** Default search result page size. */
export const SEARCH_TOP_K = 30;

/** GUI runs list limit. */
export const RUNS_LIST_LIMIT = 40;

/** Model scoreboard window (days). */
export const SCOREBOARD_WINDOW_DAYS = 7;

/** Model list fetch limit (composer + harness). */
export const MODEL_LIST_LIMIT = 80;

/** Loquela tier picker shows first N models from registry. */
export const LOQUELA_TIER_MODEL_COUNT = 24;

/** Loquela @file picker debounce (ms). */
export const LOQUELA_FILE_PICKER_DEBOUNCE_MS = 200;

/** Loquela @file picker suggestion cap. */
export const LOQUELA_FILE_PICKER_LIMIT = 20;

/** Live indicator: orch-status event considered fresh within this window (ms). */
export const LIVE_EVENT_FRESH_MS = 10_000;

/** Dockview layout persistence debounce (ms). */
export const LAYOUT_PERSIST_DEBOUNCE_MS = 1000;
