/**
 * Typed Tauri invoke payloads shared across the GUI shell.
 * Wire shapes mirror Rust command DTOs in `crates/vox-gui/src/commands/`.
 */

import type { CommandCatalogEntry } from './catalog';
import type { Agent, LudusAlert, Peer, StreamItem } from './dashboard';

export type { Agent, StreamItem, LudusAlert };
export type CatalogEntry = CommandCatalogEntry;

/** Toast input before the shell assigns an `id`. */
export type Toast = {
  tone: 'ok' | 'warn' | 'info';
  title: string;
  body?: string;
  cmd?: string;
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
  kind: 'file' | 'web' | 'memory' | 'chat' | 'command' | 'none';
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
