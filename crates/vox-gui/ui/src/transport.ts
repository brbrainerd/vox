import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ActionManifest } from './types/actionManifest';
import type {
  CommandCatalog,
  OpenLocator,
  OpenOutcome,
  OrchestratorStatus,
  RoutingSummary,
} from './types/tauri';
import type { TaskRow } from './components/surfaces/Tasks/tasksHelpers';

// `OpenLocator` / `OpenOutcome` (the `open_locator` IPC DTOs) live in ./types/tauri
// alongside the other Tauri command types; re-exported here for callers of the hub.
export type { OpenLocator, OpenOutcome } from './types/tauri';

/** Tauri event name carrying the orchestrator status snapshot (see B1 daemon stream). */
export const ORCH_STATUS_EVENT = 'vox://orch-status';

/**
 * Subscribe to the pushed orchestrator-status event stream. The payload is the
 * same status object shape returned by `get_orchestrator_status` / daemon
 * `orch.status()` (fields like `agent_count`). Returns the `UnlistenFn` to call
 * on cleanup. Rejects if not running inside Tauri (caller should fall back to polling).
 */
export function listenOrchStatus(
  onStatus: (status: OrchestratorStatus) => void,
): Promise<UnlistenFn> {
  return listen<OrchestratorStatus>(ORCH_STATUS_EVENT, (event) => onStatus(event.payload));
}

/** Tauri event name carrying a single live AgentEvent (see B4 daemon stream). */
export const AGENT_EVENTS_EVENT = 'vox://agent-events';

/**
 * A serialized `AgentEvent` value as pushed by the daemon's
 * `orch.subscribe_events` stream. The `kind.type` discriminator is a snake_case
 * variant name (e.g. "token_streamed", "task_started"); the remaining `kind`
 * fields vary per variant.
 */
export interface AgentEventFrame {
  id: number;
  timestamp_ms: number;
  kind: { type: string; [k: string]: any };
}

/**
 * Subscribe to the pushed live agent-event stream (B4). Each emission carries
 * one `AgentEventFrame`. Returns the `UnlistenFn` to call on cleanup. Rejects if
 * not running inside Tauri (caller should degrade gracefully).
 */
export function listenAgentEvents(
  onEvent: (e: AgentEventFrame) => void,
): Promise<UnlistenFn> {
  return listen<AgentEventFrame>(AGENT_EVENTS_EVENT, (event) => onEvent(event.payload));
}

/** Tauri event name carrying a Scientia-queue change ping (see F2 DB watcher). */
export const SCIENTIA_QUEUE_EVENT = 'vox://scientia-queue';

/**
 * Compact payload pushed when the Scientia queue changes. It is a *signal*, not
 * the queue itself: on receipt the UI refetches via the typed read commands.
 */
export interface ScientiaQueuePing {
  signal: number;
  manifest_count: number;
  research_count: number;
}

/**
 * Subscribe to the pushed Scientia-queue change stream (F2). The Rust side polls
 * the canonical DB and emits only when the queue signal flips, so each callback
 * means "something changed — refetch". Returns the `UnlistenFn` for cleanup.
 * Rejects if not running inside Tauri (caller should keep its interval fallback).
 */
export function listenScientiaQueue(
  onChange: (ping: ScientiaQueuePing) => void,
): Promise<UnlistenFn> {
  return listen<ScientiaQueuePing>(SCIENTIA_QUEUE_EVENT, (event) => onChange(event.payload));
}

/** Tauri event for one newly-surfaced discovery inbox row (mirrors `scientia.discovery.surfaced`). */
export const SCIENTIA_DISCOVERY_SURFACED_EVENT = 'vox://scientia-discovery-surfaced';

/** One discovery inbox row pushed when a candidate surfaces. */
export interface DiscoverySurfacedPayload {
  id: number;
  publication_id: string;
  surfaced_at_ms: number;
  intake_tier: string;
  signal_codes: string[];
  /** `research` when signal codes include `research_pipeline.*`, else `commit_watcher`. */
  origin: string;
}

/**
 * Subscribe to newly-surfaced discovery candidates. Each emission is one inbox row;
 * refetch or merge locally on receipt. Rejects outside Tauri (interval fallback).
 */
export function listenDiscoverySurfaced(
  onRow: (row: DiscoverySurfacedPayload) => void,
): Promise<UnlistenFn> {
  return listen<DiscoverySurfacedPayload>(SCIENTIA_DISCOVERY_SURFACED_EVENT, (event) =>
    onRow(event.payload),
  );
}

/** Tauri event name carrying browser live-view PNG frames (CDP mirror). */
export const BROWSER_FRAME_EVENT = 'vox://browser-frame';
export const PREVIEW_AVAILABLE_EVENT = 'vox://preview-available';

export interface BrowserFramePayload {
  timestamp_ms: number;
  page_id: string | null;
  image_base64: string | null;
  viewport_width: number | null;
  viewport_height: number | null;
  action_log: string[];
  error: string | null;
}

export interface BrowserPageSummary {
  page_id: string;
  url: string;
  title: string;
}

export interface BrowserPageInfo {
  page_id: string;
  url: string;
  title: string;
  can_go_back: boolean;
  can_go_forward: boolean;
}

export interface PreviewAvailablePayload {
  url: string;
  app_dir: string | null;
  source: string;
}

/**
 * Subscribe to pushed browser frame snapshots (~3s when a session is active).
 */
export function listenBrowserFrames(
  onFrame: (frame: BrowserFramePayload) => void,
): Promise<UnlistenFn> {
  return listen<BrowserFramePayload>(BROWSER_FRAME_EVENT, (event) => onFrame(event.payload));
}

export function listenPreviewAvailable(
  onPreview: (payload: PreviewAvailablePayload) => void,
): Promise<UnlistenFn> {
  return listen<PreviewAvailablePayload>(PREVIEW_AVAILABLE_EVENT, (event) => onPreview(event.payload));
}

/** Tauri event name emitted when the secretary auto-submits a task from chat. */
export const SECRETARY_PROPOSED_EVENT = 'vox://secretary-proposed-task';

export interface SecretaryProposedPayload {
  item_id: string;
  intent: string;
  confidence_pct: number;
}

/**
 * Subscribe to the secretary proposed task event.
 */
export function listenSecretaryProposed(
  onProposed: (payload: SecretaryProposedPayload) => void,
): Promise<UnlistenFn> {
  return listen<SecretaryProposedPayload>(SECRETARY_PROPOSED_EVENT, (event) => onProposed(event.payload));
}


export interface ExecuteOutput {
  exit_code: number;
  stdout: string;
  stderr: string;
}

/** Mirrors Rust `IdentitySummaryDto` from `commands/identity.rs`. */
export interface IdentitySummary {
  display_name: string;
  os_user?: string | null;
}

/** Mirrors Rust `LlmSpendDto` from `commands/user_config.rs` (camelCase on wire). */
export interface LlmSpendDto {
  sessionUsd: number;
  dayUsd: number;
  totalUsd: number;
  dailyBudgetUsd: number;
  perSessionBudgetUsd: number;
}

export interface GamifySettingsDto {
  enabled: boolean;
  mode: string;
}

/** Wire DTO returned by `record_gui_event` (camelCase on the Tauri bridge). */
export interface GuiEventResultDto {
  xpGranted: number;
  lumensGranted: number;
  achievementTitle?: string | null;
}

export interface CommandMetadata {
    product_lane: string | null;
    feature_gate: string | null;
    catalog_group: string | null;
    status: string;
}

export interface RegistryOperation {
  path: string[];
  status: string;
  product_lane: string | null;
  feature_gate: string | null;
  catalog_group: string | null;
  surface: string;
}

export interface RegistryFile {
  schema_version: number;
  operations: RegistryOperation[];
}

/** Resolved set of operations keyed by underscore-joined path for O(1) lookup. */
type RegistryIndex = Map<string, RegistryOperation>;

class VoxTransport {
  private registryCache: RegistryFile | null = null;
  private registryIndex: RegistryIndex | null = null;
  /** Singleton promise so concurrent callers don't double-fetch. */
  private registryFetch: Promise<RegistryFile> | null = null;
  private actionManifestCache: ActionManifest | null = null;
  private actionManifestFetch: Promise<ActionManifest> | null = null;

  async getRegistry(): Promise<RegistryFile> {
    if (this.registryCache) return this.registryCache;
    if (!this.registryFetch) {
      this.registryFetch = invoke<RegistryFile>('get_full_registry').then(r => {
        this.registryCache = r;
        // Build an index for fast lookups.
        this.registryIndex = new Map(
          r.operations.map(op => [op.path.join('_'), op])
        );
        return r;
      });
    }
    return this.registryFetch;
  }

  /** Invalidate caches — call when the registry may have changed on disk. */
  invalidateRegistry() {
    this.registryCache = null;
    this.registryIndex = null;
    this.registryFetch = null;
    this.actionManifestCache = null;
    this.actionManifestFetch = null;
  }

  async getActionManifest(): Promise<ActionManifest> {
    if (this.actionManifestCache) return this.actionManifestCache;
    if (!this.actionManifestFetch) {
      this.actionManifestFetch = invoke<ActionManifest>('get_action_manifest').then((m) => {
        this.actionManifestCache = m;
        return m;
      });
    }
    return this.actionManifestFetch;
  }

  /** Return all operations for a given product_lane (e.g. "platform", "app"). */
  async getOperationsByLane(lane: string): Promise<RegistryOperation[]> {
    const reg = await this.getRegistry();
    return reg.operations.filter(op => op.product_lane === lane);
  }

  /** Return all operations for a given feature_gate. */
  async getGatedOperations(gate: string): Promise<RegistryOperation[]> {
    const reg = await this.getRegistry();
    return reg.operations.filter(op => op.feature_gate?.includes(gate));
  }

  async resolvePath(actionId: string): Promise<string[]> {
    await this.getRegistry(); // ensures index is built
    const cleanId = actionId.startsWith('vox_') ? actionId.substring(4) : actionId;

    // 1. Exact match on underscore-joined path.
    if (this.registryIndex?.has(cleanId)) {
      return this.registryIndex.get(cleanId)!.path;
    }

    // 2. Try with dashes (CLI convention).
    const dashId = cleanId.replace(/_/g, '-');
    for (const [key, op] of this.registryIndex ?? []) {
      if (op.path.join('-') === dashId) return op.path;
    }

    // 3. Prefix-aware fallback for orchestrator/dei/gamify namespaces.
    const parts = cleanId.split('_');
    if (parts[0] === 'dei' || parts[0] === 'orchestrator') {
      return ['dei', ...parts.slice(1).map(p => p.replace(/_/g, '-'))];
    }
    if (parts[0] === 'gamify') {
      return ['ludus', ...parts.slice(1).map(p => p.replace(/_/g, '-'))];
    }

    return [cleanId.replace(/_/g, '-')];
  }

  async getCatalog(): Promise<CommandCatalog> {
    return invoke<CommandCatalog>('get_command_catalog');
  }

  async listModels(limit = 120) {
    return invoke('list_model_cards', { limit });
  }

  async getActiveModel() {
    return invoke<string | null>('get_active_model');
  }

  async setActiveModel(modelId: string) {
    return invoke('set_active_model', { modelId });
  }

  async getRoutingSummaryLive(): Promise<RoutingSummary> {
    return invoke<RoutingSummary>('get_routing_summary_live');
  }

  async listOrchestratorTasks(): Promise<TaskRow[]> {
    return invoke<TaskRow[]>('list_orchestrator_tasks');
  }

  async setRoutingPriority(priority: {
    efficiency: number;
    precision: number;
    latency: number;
    availability: number;
    balance: number;
    mobile: number;
  }) {
    return invoke('set_routing_priority', priority);
  }

  /** Read the persisted selection-policy JSON (`{"steps":[...]}`). */
  async getSelectionPolicy(): Promise<string> {
    return invoke<string>('get_selection_policy');
  }

  /** Persist a selection-policy JSON; backend validates it parses as SelectionPolicy. */
  async setSelectionPolicy(json: string): Promise<void> {
    return invoke('set_selection_policy', { json });
  }

  async getModelScoreboard(windowDays = 7) {
    return invoke('get_model_scoreboard', { windowDays });
  }

  async explainModelSelection(task: string, complexity?: number) {
    return invoke('explain_model_selection', { task, complexity });
  }

  async suggestModelForTask(task: string) {
    return invoke('suggest_model_for_task', { task });
  }

  async callTool(name: string, args: Record<string, any> = {}): Promise<ExecuteOutput> {
    if (name === 'vox_list_models') {
      const models = await this.listModels(args.limit ?? 120);
      return { exit_code: 0, stdout: JSON.stringify(models), stderr: '' };
    }
    if (name === 'vox_set_active_model' && args.model_id) {
      await this.setActiveModel(String(args.model_id));
      return { exit_code: 0, stdout: 'ok', stderr: '' };
    }
    if (name === 'vox_explain_model' && args.task) {
      const out = await this.explainModelSelection(
        String(args.task),
        Number(args.intelligence ?? 50),
      );
      return { exit_code: 0, stdout: JSON.stringify(out), stderr: '' };
    }
    if (name === 'vox_suggest_model' && args.task) {
      const out = await this.suggestModelForTask(String(args.task));
      return { exit_code: 0, stdout: JSON.stringify(out), stderr: '' };
    }
    const manifest = await this.getActionManifest();
    const canonical = name.startsWith('vox_') ? name : `vox_${name}`;
    const action = manifest.actions.find((a) =>
      a.mcp_name === canonical ||
      a.id === name ||
      a.id === canonical.replace(/^vox_/, '').replace(/_/g, '.') ||
      a.command === `vox ${name.replace(/^vox_/, '').replace(/_/g, ' ')}`
    );
    if (action?.handler_kind === 'mcp') {
      const tool = action.mcp_name ?? canonical;
      const result = await invoke<any>('invoke_mcp_tool', { tool, args });
      const isError =
        result != null &&
        typeof result === 'object' &&
        (result as { is_error?: boolean }).is_error === true;
      return {
        exit_code: isError ? 1 : 0,
        stdout: JSON.stringify(result),
        stderr: '',
      };
    }
    const path = action?.cli_path ?? (await this.resolvePath(name));
    const res = await invoke<ExecuteOutput>('execute_command', {
      path,
      args: { ...args, __argv: args.__argv ?? [] },
    });
    return res;
  }

  async getMetadata(path: string[]): Promise<CommandMetadata | null> {
    return invoke('get_command_metadata', { path });
  }

  logFrontend(level: 'error' | 'warn' | 'info', message: string): Promise<void> {
    return invoke('log_frontend', { level, message });
  }

  getGuiPreference(key: string): Promise<string | null> {
    return invoke<string | null>('get_gui_preference', { key });
  }

  setGuiPreference(key: string, value: string): Promise<void> {
    return invoke('set_gui_preference', { key, value });
  }

  invokeMcpTool(
    tool: string,
    args: Record<string, unknown> = {},
  ): Promise<{ is_error?: boolean; result?: unknown }> {
    return invoke('invoke_mcp_tool', { tool, args });
  }

  openLocator(locator: OpenLocator): Promise<OpenOutcome> {
    return invoke<OpenOutcome>('open_locator', { locator });
  }

  voxDocsIndex(): Promise<{ title: string; description: string; path: string }[]> {
    return invoke('vox_docs_index');
  }

  /** VG-1 build-time GUI content manifest (gui-content-manifest.json). */
  voxContentManifest(): Promise<import('./hooks/useContentManifest').ContentManifestEntry[]> {
    return invoke('vox_content_manifest');
  }

  /** Policy catalog rows for federated OmniSearch (see policy_list IPC). */
  listPolicies(): Promise<{ name: string; status?: string }[]> {
    return invoke<{ id: string }[]>('policy_list', { domain: null, group: null }).then(rows => {
      if (!Array.isArray(rows)) return [];
      return rows.map(r => ({ name: r.id }));
    });
  }

  voxSearchQuery(query: string, limit: number, scope: string[]): Promise<{
    hits: unknown[];
    facets_by_source: { value: string; count: number }[];
    facets_by_kind: { value: string; count: number }[];
    total: number;
    next_cursor: number | null;
    corpora: string[];
    repo_truncated: boolean;
  }> {
    return invoke('vox_search_query', { query, limit, scope });
  }

  /** Raw MessagePack orchestrator snapshot (same payload as `get_orchestrator_status`). */
  getOrchestratorStatusBin(): Promise<Uint8Array> {
    return invoke<Uint8Array>('get_orchestrator_status_bin');
  }

  getIdentitySummary(): Promise<IdentitySummary> {
    return invoke<IdentitySummary>('get_identity_summary');
  }

  getLlmSpend(sessionId?: string | null): Promise<LlmSpendDto> {
    return invoke<LlmSpendDto>('get_llm_spend', sessionId != null ? { sessionId } : {});
  }

  getGamifySettings(): Promise<GamifySettingsDto> {
    return invoke<GamifySettingsDto>('get_gamify_settings');
  }

  recordGuiEvent(
    eventType: string,
    metadata?: Record<string, unknown>,
  ): Promise<GuiEventResultDto> {
    return invoke<GuiEventResultDto>('record_gui_event', {
      eventType,
      metadata: metadata ?? null,
    });
  }

  getMemoryStatus(): Promise<{ corpus_counts: Record<string, number> }> {
    return invoke<{ corpus_counts: Record<string, number> }>('get_memory_status');
  }

  doubtTask(taskId: number, reason?: string): Promise<unknown> {
    return invoke('doubt_orchestrator_task', { taskId, reason: reason ?? null });
  }

  overruleTask(taskId: number, reason: string): Promise<unknown> {
    return invoke('overrule_orchestrator_task', { taskId, reason });
  }

  mercatusLoadConfig(): Promise<unknown> {
    return invoke('mercatus_load_config');
  }

  mercatusSaveConfig(config: unknown): Promise<void> {
    return invoke('mercatus_save_config', { config });
  }
}

export const voxTransport = new VoxTransport();

// ---------------------------------------------------------------------------
// Vox Console: discovery engine + PTY terminal transport wrappers.
// ---------------------------------------------------------------------------

export interface Suggestion {
  action_id: string;
  completion: string;
  about: string;
}

export interface ActionHelp {
  action_id: string;
  about: string;
  args: { name: string; help: string; required: boolean }[];
  example: string;
}

export function discoverySuggest(typed: string, limit = 8): Promise<Suggestion[]> {
  return invoke<Suggestion[]>('discovery_suggest', { typed, limit });
}

export function discoveryHelp(actionId: string): Promise<ActionHelp | null> {
  return invoke<ActionHelp | null>('discovery_help', { actionId });
}

export function discoveryRecord(
  actionId: string,
  used: boolean,
  nowMs: number,
  dwellMs: number,
): Promise<void> {
  return invoke('discovery_record', { actionId, used, nowMs, dwellMs });
}

export function ptySpawn(tabId: string, cols: number, rows: number): Promise<void> {
  return invoke('pty_spawn', { tabId, cols, rows });
}

export function ptyWrite(tabId: string, data: string): Promise<void> {
  return invoke('pty_write', { tabId, data });
}

export function ptyClose(tabId: string): Promise<void> {
  return invoke('pty_close', { tabId });
}

export const PTY_OUTPUT_EVENT = 'vox://pty-output';
export const PTY_EXIT_EVENT = 'vox://pty-exit';

export function listenPtyOutput(
  onChunk: (tabId: string, data: string) => void,
): Promise<UnlistenFn> {
  return listen<{ tab_id: string; data: string }>(PTY_OUTPUT_EVENT, (e) =>
    onChunk(e.payload.tab_id, e.payload.data),
  );
}

export function listenPtyExit(onExit: (tabId: string) => void): Promise<UnlistenFn> {
  return listen<{ tab_id: string }>(PTY_EXIT_EVENT, (e) => onExit(e.payload.tab_id));
}

// ---------------------------------------------------------------------------
// Policy enable/disable + edit transport wrappers.
// ---------------------------------------------------------------------------

export function policySetEnabled(id: string, enabled: boolean): Promise<void> {
  return invoke('policy_set_enabled', { id, enabled });
}

export function policyEdit(id: string, title?: string, description?: string): Promise<void> {
  return invoke('policy_edit', { id, title: title ?? null, description: description ?? null });
}

/** Send a free-form note to an agent's A2A inbox. Resolves to the message id. */
export function sendToAgent(agentId: string, body: string): Promise<string> {
  return invoke<string>('send_to_agent', { agentId, body });
}

export interface ContextBudgetPayload {
  max_context_tokens: number;
  reserved_tokens: number;
  threshold_tokens: number;
  usable_tokens: number;
  strategy: string;
  /** Cumulative input+output tokens used in the session from llm_interactions. Zero when no session. */
  used_tokens: number;
}

export function getContextBudget(sessionId?: string | null): Promise<ContextBudgetPayload> {
  return invoke<ContextBudgetPayload>('get_context_budget', sessionId != null ? { sessionId } : {});
}

export interface ActivityRowDto {
  id: number;
  ts_ms: number;
  agent_id?: string;
  session_id?: string;
  kind: string;
  summary: string;
  detail_json: string;
}

export interface ActivityFilterDto {
  agent_id: string | null;
  kind: string | null;
  limit: number;
  before_id: number | null;
}

export function activityQuery(filter: ActivityFilterDto): Promise<ActivityRowDto[]> {
  return invoke<ActivityRowDto[]>('activity_query', { filter });
}

export const ACTIVITY_APPENDED_EVENT = 'vox://activity-appended';

export function listenActivityAppended(onAppend: () => void): Promise<UnlistenFn> {
  return listen<void>(ACTIVITY_APPENDED_EVENT, () => onAppend());
}

// `getGraphifyStatus` (direct `vox_graphify_status` Tauri command) retired in
// T8 — the GUI now reads status through `useGraphifyStatus` →
// `voxTransport.invokeMcpTool('vox_search_status')` (the shared MCP dispatch).

export interface FeedbackRow {
  feedbackId: string;
  kind: 'clarification' | 'doubt' | 'skill_proposal';
  prompt: string;
  options: string[];
  gates: number[];
  doubtedTaskId: number | null;
  surface: 'needs_you' | 'withheld';
  infoGainBits: number;
}

const toRow = (r: any): FeedbackRow => ({
  feedbackId: r.id,
  kind: r.kind,
  prompt: r.prompt,
  options: r.options ?? [],
  gates: r.gates ?? [],
  doubtedTaskId: r.doubted_task_id ?? null,
  surface: r.surface,
  infoGainBits: r.info_gain_bits ?? 0,
});

export function normalizeFeedback(raw: any): { needsYou: FeedbackRow[]; withheld: FeedbackRow[] } {
  const ny = (raw?.needs_you ?? []).map(toRow).sort((a: FeedbackRow, b: FeedbackRow) => {
    if (a.kind !== b.kind) return a.kind === 'doubt' ? -1 : 1; // doubts pinned top
    return b.infoGainBits - a.infoGainBits;
  });
  return { needsYou: ny, withheld: (raw?.withheld ?? []).map(toRow) };
}

export async function feedbackList(): Promise<{ needsYou: FeedbackRow[]; withheld: FeedbackRow[] }> {
  const res = await invoke<string>('invoke_mcp_tool', { tool: 'vox_feedback_list', args: {} });
  const parsed = JSON.parse(res);
  if (!parsed.success) {
    throw new Error(parsed.error || 'Failed to list feedback');
  }
  return normalizeFeedback(parsed.data);
}

export async function feedbackResolve(feedbackId: string, action: Record<string, unknown>): Promise<void> {
  const res = await invoke<string>('invoke_mcp_tool', {
    tool: 'vox_resolve_feedback',
    args: { feedback_id: feedbackId, action }
  });
  const parsed = JSON.parse(res);
  if (!parsed.success) {
    throw new Error(parsed.error || 'Failed to resolve feedback');
  }
}

export function listenFeedbackChanged(onChange: () => void): Promise<UnlistenFn> {
  return listen<any>(AGENT_EVENTS_EVENT, (e) => {
    const t = e?.payload?.kind?.type;
    if (t === 'feedback_requested' || t === 'feedback_resolved') onChange();
  });
}



