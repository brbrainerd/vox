/**
 * Typed Tauri invoke payloads shared across the GUI shell.
 * Wire shapes mirror Rust command DTOs in `crates/vox-gui/src/commands/`.
 */

import type { CommandCatalogEntry } from './catalog';
import type { Agent, LudusAlert, Peer, StreamItem } from './dashboard';

export type { Agent, StreamItem, LudusAlert };
export type CatalogEntry = CommandCatalogEntry;

export type ToastCause =
  | 'backend-ok'      // an async Tauri command / mutation succeeded
  | 'backend-error'   // an async Tauri command / mutation failed
  | 'validation'      // user input rejected before any effect
  | 'clipboard'       // copied to clipboard (real OS effect)
  | 'external';       // opened an external app/url
// NOTE: deliberately NO cause for navigation or a routine, already-visible synchronous
// action — those must NOT toast. A toast with no honest cause is a compile error.

/** Toast input before the shell assigns an `id`. */
export type Toast = {
  tone: 'ok' | 'warn' | 'info';
  title: string;
  body?: string;
  cmd?: string;
  cause: ToastCause; // required
  /**
   * Optional coalescing identity: a toast that arrives while another
   * visible toast shares the same `groupKey` merges into it (shown with a
   * count) instead of adding a new entry. Defaults to `title` when omitted.
   */
  groupKey?: string;
};

/** Mirrors `ChatSessionDto` from `commands/chat.rs`. */
export interface Session {
  session_id: string;
  title: string;
  updated_at: string;
  message_count: number;
  conversation_id: number;
}

/** Mirrors `SubmitTaskInput` / Loquela composer dispatch payload. */
export interface ChatPayload {
  description: string;
  session_id?: string;
  priority?: string | null;
  mode?: string | null;
  model_hint?: string | null;
  tier?: string | null;
  dry_run?: boolean | null;
  active_skill?: string | null;
  files?: string[];
  /** Drive Console clutch detent — how aggressively to spend on models. Track D wires this to the orchestrator. */
  clutch?: string | null;
  /** Drive Console risk posture — safety gate level. Track D wires this to the orchestrator. */
  risk?: string | null;
  /** Explicit model pick for this submit; maps to the model_override enqueue hint. */
  model_override?: string | null;
  /**
   * Explicit task-category hint. `'chat'` short-circuits CLIENT-SIDE
   * (App.tsx::handleLoquelaSubmit) to the synchronous chat_send_message
   * command, before submit_orchestrator_task is ever called -- this is not
   * daemon-side routing, the daemon never sees a `'chat'`-tagged submission
   * from this path. Set by the composer's "Quick chat" send-mode toggle
   * (Loquela.tsx's `executionMode`), the default. Left `undefined` for the
   * "Background task" toggle position and for other agentic submissions
   * (e.g. `/spawn`, skill deploy), which take the normal
   * submit_orchestrator_task -> AiTaskProcessor dispatch path so the daemon
   * falls back to its default category resolution.
   */
  task_category?: string | null;
  /**
   * Opt-in, per-session grounding/hallucination-check toggle from the chat
   * composer (see `hooks/useGroundingCheck.ts`). `undefined`/`null` leaves
   * the daemon's default (off) in place.
   */
  grounding_check_enabled?: boolean | null;
}

export interface RoutingPriority {
  efficiency: number;
  precision: number;
  latency: number;
  availability: number;
  balance: number;
  mobile: number;
}

export interface DecisionPreview {
  selected_model: string;
  discovery_state: string;
  alternatives: string[];
  rejection_reasons: string[];
  intelligence_score: number;
  efficiency_score: number;
  latency_score: number;
}

/** Mirrors `RoutingSummaryDto` from `commands/models.rs`. */
export interface RoutingSummary {
  active_model: string | null;
  exploration_spent_usd: number;
  exploration_budget_usd: number;
  routing_priority: RoutingPriority;
  arm_count: number;
  model_count: number;
  decision_preview: DecisionPreview | null;
}

/** Raw agent row from orchestrator status before `mapAgent`. */
export interface RawAgentSummary {
  id: number | string;
  codename?: string;
  name?: string;
  in_progress?: boolean;
  paused?: boolean;
  progress?: number | null;
  current_phase?: string;
  task_description?: string;
  cost?: number;
  budget?: number | null;
  eta?: string;
  active_skill?: string;
}

/** Raw stream event from orchestrator status before `mapStream`. */
export interface RawStreamEvent {
  id?: number | string;
  kind?: StreamItem['kind'];
  tag?: string;
  title?: string;
  body?: string;
  timestamp?: string;
}

/** Raw Ludus alert from orchestrator status before `mapAlert`. */
export interface RawLudusAlert {
  id: string;
  level: LudusAlert['level'];
  title: string;
  body: string;
}

/** Orchestrator status snapshot (msgpack/JSON wire shape). */
export interface OrchestratorStatus {
  agent_count?: number;
  total_queued?: number;
  total_in_progress?: number;
  total_completed?: number;
  total_doubted?: number;
  total_weighted_load?: number;
  predicted_load?: number;
  agents?: RawAgentSummary[];
  recent_events?: RawStreamEvent[];
  alerts?: RawLudusAlert[];
  peers?: Peer[];
  total_cost?: number;
  budget_cap?: number;
  mesh_throughput?: number;
  total_vram_gb?: number;
  /** Present when the daemon reports the live attention budget (Track D). */
  attention_budget?: AttentionBudgetSnapshot | null;
}

/** Flat attention-budget snapshot (Rust `AttentionBudget`), surfaced via the orchestrator status stream. */
export interface AttentionBudgetSnapshot {
  max_attention_ms: number;
  spent_ms: number;
  total_requests: number;
  auto_approved: number;
  rejected: number;
  interrupt_freq_per_hour: number;
  last_interrupt_ms: number;
  inbox_suppressed_count: number;
}


export interface CommandCatalog {
  generated_from?: string;
  entries: CatalogEntry[];
}

export interface ActiveSkill {
  id: string;
  name?: string;
  command?: string;
}

export interface ContextChip {
  id: string;
  kind: 'file' | 'skill' | 'agent' | 'branch' | 'url' | 'image';
  label: string;
  meta?: string;
}

/**
 * A locator the Rust `open_locator` command can act on. Mirrors the backend
 * `OpenLocatorDto { kind, value }` (see `crates/vox-gui/src/commands/search.rs`).
 * `kind` selects the handler (file → editor, web → browser); other kinds are no-ops.
 */
export interface OpenLocator {
  /** `setting` is GUI-only (Search/palette seed); not handled by Rust `open_locator`. */
  kind: 'file' | 'web' | 'memory' | 'chat' | 'command' | 'setting' | 'none';
  value: string;
}

/** Outcome of an `open_locator` call. Mirrors the backend `OpenOutcomeDto`. */
export interface OpenOutcome {
  /** "spawned" (launched an external app) or "opened". */
  action: string;
}

/** Union accepted by the command palette `onAction` handler. */
export type CommandPaletteAction =
  | Agent
  | CatalogEntry
  | { id: 'submit' | 'search' | 'pause-all' | 'resume-all' | 'ack-all' }
  | { id: string; type?: 'navigate' | 'agent' | 'command' | 'hit'; viewKey?: string; locator?: OpenLocator; label?: string };

export interface SubmitTaskResult {
  ok: boolean;
  message: string;
  task_id: string | null;
  /** Set when the daemon refused a near-duplicate; the id of the existing task. */
  duplicate_of?: string | null;
}

export interface CorpusStatusDto {
  corpus_id: string;
  title: string;
  graph_exists: boolean;
  manifest_exists: boolean;
  node_count: number | null;
  edge_count: number | null;
  built_at: string | null;
  manifest_git_sha: string | null;
  head_git_sha: string | null;
  stale_reasons: string[];
  warnings: string[];
  is_fresh: boolean;
}

export interface GraphifyStatusDto {
  default_corpus_id: string;
  /** Effective staleness TTL in days after env > contract precedence.
   *  Optional: older backends omit it, and the editor hides itself when absent. */
  ttl_days?: number;
  /** True when VOX_GRAPHIFY_TTL_DAYS is forcing `ttl_days`, so a stored
   *  contract value would have no local effect. */
  ttl_days_env_forced?: boolean;
  corpora: CorpusStatusDto[];
}

