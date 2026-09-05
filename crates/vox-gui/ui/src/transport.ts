import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { backendAvailable, BackendUnavailableError } from './lib/backendGuard';
import type { ActionManifest } from './types/actionManifest';
import type {
  CommandCatalog,
  OpenLocator,
  OpenOutcome,
  OrchestratorStatus,
  RoutingSummary,
} from './types/tauri';
import type { TaskRow } from './components/surfaces/Tasks/tasksHelpers';
import type { TownScan } from './components/gamify/urbs/types';
import type { TurnEventDto } from './types/dashboard';

// `OpenLocator` / `OpenOutcome` (the `open_locator` IPC DTOs) live in ./types/tauri
// alongside the other Tauri command types; re-exported here for callers of the hub.
export type { OpenLocator, OpenOutcome } from './types/tauri';

/** A single category's pass/fail tally within a harness eval run (Harness Health surface). */
export interface CategorySummaryDto {
  category: string;
  pass_count: number;
  fail_count: number;
}

/** One persisted `vox harness eval --live` run, as returned by `harness_eval_history`. */
export interface HarnessEvalRunDto {
  run_id: string;
  git_sha: string;
  triggered_by: string;
  pass_count: number;
  fail_count: number;
  skip_count: number;
  total_cost_usd: number;
  started_at_ms: number;
  category_breakdown: CategorySummaryDto[];
}

/** A detected regression between two consecutive harness eval runs. */
export interface RegressionFlagDto {
  kind: string;
  previous_run_id: string;
  current_run_id: string;
  previous_git_sha: string;
  current_git_sha: string;
  changed_files: string[];
  flipped_task_ids: string[];
  detail: string;
}

// __VOX_RAW_IPC_BEGIN__
// The ONLY permitted raw Tauri `invoke`/`listen` uses in this file.
// Guarded by src/guards/transportIpcGuard.test.ts.
function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!backendAvailable()) return Promise.reject(new BackendUnavailableError(cmd));
  return args === undefined ? invoke<T>(cmd) : invoke<T>(cmd, args);
}

function safeListen<T>(
  event: string,
  handler: (event: { payload: T }) => void,
): Promise<UnlistenFn> {
  if (!backendAvailable()) return Promise.reject(new BackendUnavailableError(`listen:${event}`));
  return listen<T>(event, handler);
}
// __VOX_RAW_IPC_END__

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
  return safeListen<OrchestratorStatus>(ORCH_STATUS_EVENT, (event) => onStatus(event.payload));
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
  return safeListen<AgentEventFrame>(AGENT_EVENTS_EVENT, (event) => onEvent(event.payload));
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
  return safeListen<ScientiaQueuePing>(SCIENTIA_QUEUE_EVENT, (event) => onChange(event.payload));
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
  return safeListen<DiscoverySurfacedPayload>(SCIENTIA_DISCOVERY_SURFACED_EVENT, (event) =>
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
  return safeListen<BrowserFramePayload>(BROWSER_FRAME_EVENT, (event) => onFrame(event.payload));
}

export function listenPreviewAvailable(
  onPreview: (payload: PreviewAvailablePayload) => void,
): Promise<UnlistenFn> {
  return safeListen<PreviewAvailablePayload>(PREVIEW_AVAILABLE_EVENT, (event) => onPreview(event.payload));
}

/**
 * Tauri event name emitted when the secretary detects actionable intent in a
 * chat message and proposes a task. Propose-only (Task 0.2): no task exists
 * yet at this point — the frontend must call `secretary_confirm_task` to
 * actually submit it.
 */
export const SECRETARY_PROPOSED_EVENT = 'vox://secretary-proposed-task';

export interface SecretaryProposedPayload {
  /** Client-side proposal id — NOT a hopper/task id, no task has been submitted yet. */
  item_id: string;
  intent: string;
  confidence_pct: number;
  /** Chat session id, passed back to `secretary_confirm_task` on confirm. */
  session_id: string;
}

/**
 * Subscribe to the secretary proposed task event.
 */
export function listenSecretaryProposed(
  onProposed: (payload: SecretaryProposedPayload) => void,
): Promise<UnlistenFn> {
  return safeListen<SecretaryProposedPayload>(SECRETARY_PROPOSED_EVENT, (event) => onProposed(event.payload));
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

/**
 * T0.3: GUI-selected `PermissionMode` wire string
 * (`"ask" | "accept_edits" | "accept_all" | "plan"`), threaded onto every
 * `invoke_mcp_tool` call this transport makes. Module-level (not per-call)
 * so any surface driving a tool call — not just the approvals view that
 * owns the toggle — picks up the currently selected mode without every
 * call site needing to know about it. `null` (the default) means "no mode
 * selected"; the Rust side treats an absent mode as the fail-safe `ask`
 * default (today's always-park behavior for dangerous tools).
 *
 * Mirrors `vox_orchestrator_mcp::permission_modes::PermissionMode`
 * (`contracts/orchestration/permission-modes.v1.yaml`).
 */
let currentPermissionMode: string | null = null;

/** Read the currently selected `PermissionMode` wire string, or `null`. */
export function getPermissionMode(): string | null {
  return currentPermissionMode;
}

/**
 * Set the `PermissionMode` wire string threaded onto subsequent
 * `invoke_mcp_tool` calls. Pass `null` (or `'ask'`) to return to the
 * fail-safe default.
 */
export function setPermissionMode(mode: string | null): void {
  currentPermissionMode = mode;
}

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
      this.registryFetch = safeInvoke<RegistryFile>('get_full_registry').then(r => {
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
      this.actionManifestFetch = safeInvoke<ActionManifest>('get_action_manifest').then((m) => {
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
    return safeInvoke<CommandCatalog>('get_command_catalog');
  }

  async listModels(limit = 120) {
    return safeInvoke('list_model_cards', { limit });
  }

  async getActiveModel() {
    return safeInvoke<string | null>('get_active_model');
  }

  async setActiveModel(modelId: string) {
    return safeInvoke('set_active_model', { modelId });
  }

  async getRoutingSummaryLive(): Promise<RoutingSummary> {
    return safeInvoke<RoutingSummary>('get_routing_summary_live');
  }

  async listOrchestratorTasks(): Promise<TaskRow[]> {
    return safeInvoke<TaskRow[]>('list_orchestrator_tasks');
  }

  async setRoutingPriority(priority: {
    efficiency: number;
    precision: number;
    latency: number;
    availability: number;
    balance: number;
    mobile: number;
  }) {
    return safeInvoke('set_routing_priority', priority);
  }

  /** Read the persisted selection-policy JSON (`{"steps":[...]}`). */
  async getSelectionPolicy(): Promise<string> {
    return safeInvoke<string>('get_selection_policy');
  }

  /** Persist a selection-policy JSON; backend validates it parses as SelectionPolicy. */
  async setSelectionPolicy(json: string): Promise<void> {
    return safeInvoke('set_selection_policy', { json });
  }

  async getModelScoreboard(windowDays = 7) {
    return safeInvoke('get_model_scoreboard', { windowDays });
  }

  async explainModelSelection(task: string, complexity?: number) {
    return safeInvoke('explain_model_selection', { task, complexity });
  }

  async suggestModelForTask(task: string) {
    return safeInvoke('suggest_model_for_task', { task });
  }

  async callTool(name: string, args: Record<string, any> = {}): Promise<ExecuteOutput> {
    if (name === 'vox_list_models') {
      const models = await this.listModels(args.limit ?? 120);
      return { exit_code: 0, stdout: JSON.stringify(models), stderr: '' };
    }
    if (name === 'vox_set_active_model' && args.model_id) {
      await this.setActiveModel(`${args.model_id}`);
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
      const result = await safeInvoke<any>('invoke_mcp_tool', {
        tool,
        args,
        permissionMode: currentPermissionMode,
      });
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
    const res = await safeInvoke<ExecuteOutput>('execute_command', {
      path,
      args: { ...args, __argv: args.__argv ?? [] },
    });
    return res;
  }

  async getMetadata(path: string[]): Promise<CommandMetadata | null> {
    return safeInvoke('get_command_metadata', { path });
  }

  logFrontend(level: 'error' | 'warn' | 'info', message: string): Promise<void> {
    return safeInvoke('log_frontend', { level, message });
  }

  getGuiPreference(key: string): Promise<string | null> {
    return safeInvoke<string | null>('get_gui_preference', { key });
  }

  setGuiPreference(key: string, value: string): Promise<void> {
    return safeInvoke('set_gui_preference', { key, value });
  }

  invokeMcpTool(
    tool: string,
    args: Record<string, unknown> = {},
  ): Promise<{ is_error?: boolean; result?: unknown }> {
    return safeInvoke('invoke_mcp_tool', {
      tool,
      args,
      permissionMode: currentPermissionMode,
    });
  }

  openLocator(locator: OpenLocator): Promise<OpenOutcome> {
    return safeInvoke<OpenOutcome>('open_locator', { locator });
  }

  voxDocsIndex(): Promise<{ title: string; description: string; path: string }[]> {
    return safeInvoke('vox_docs_index');
  }

  readDocMarkdown(path: string): Promise<string> {
    return safeInvoke('read_doc_markdown', { path });
  }

  /** Recent chat harness eval runs (Harness Health surface). */
  harnessEvalHistory(limit = 50): Promise<HarnessEvalRunDto[]> {
    return safeInvoke('harness_eval_history', { limit });
  }

  /** Detected regressions between consecutive chat harness eval runs. */
  harnessEvalRegressions(): Promise<RegressionFlagDto[]> {
    return safeInvoke('harness_eval_regressions');
  }

  /** VG-1 build-time GUI content manifest (gui-content-manifest.json). */
  voxContentManifest(): Promise<import('./hooks/useContentManifest').ContentManifestEntry[]> {
    return safeInvoke('vox_content_manifest');
  }

  /** Policy catalog rows for federated OmniSearch (see policy_list IPC). */
  listPolicies(): Promise<{ name: string; status?: string }[]> {
    return safeInvoke<{ id: string }[]>('policy_list', { domain: null, group: null }).then(rows => {
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
    return safeInvoke('vox_search_query', { query, limit, scope });
  }

  /** Raw MessagePack orchestrator snapshot (same payload as `get_orchestrator_status`). */
  getOrchestratorStatusBin(): Promise<Uint8Array> {
    return safeInvoke<Uint8Array>('get_orchestrator_status_bin');
  }

  /**
   * Daemon/GUI version mismatch cached by `PersistentDaemon`, or `null` if
   * none. The Rust side serializes `VersionMismatch` as a named
   * `{ daemonVersion, guiVersion }` object (not a positional tuple), so no
   * index-based mapping is needed here — the wire shape already matches.
   */
  getOrchestratorVersionMismatch(): Promise<{ daemonVersion: string; guiVersion: string } | null> {
    return safeInvoke<{ daemonVersion: string; guiVersion: string } | null>(
      'orchestrator_version_mismatch'
    );
  }

  getIdentitySummary(): Promise<IdentitySummary> {
    return safeInvoke<IdentitySummary>('get_identity_summary');
  }

  getLlmSpend(sessionId?: string | null): Promise<LlmSpendDto> {
    return safeInvoke<LlmSpendDto>('get_llm_spend', sessionId != null ? { sessionId } : {});
  }

  getGamifySettings(): Promise<GamifySettingsDto> {
    return safeInvoke<GamifySettingsDto>('get_gamify_settings');
  }

  recordGuiEvent(
    eventType: string,
    metadata?: Record<string, unknown>,
  ): Promise<GuiEventResultDto> {
    return safeInvoke<GuiEventResultDto>('record_gui_event', {
      eventType,
      metadata: metadata ?? null,
    });
  }

  getMemoryStatus(): Promise<{ corpus_counts: Record<string, number> }> {
    return safeInvoke<{ corpus_counts: Record<string, number> }>('get_memory_status');
  }

  doubtTask(taskId: number, reason?: string): Promise<unknown> {
    return safeInvoke('doubt_orchestrator_task', { taskId, reason: reason ?? null });
  }

  overruleTask(taskId: number, reason: string): Promise<unknown> {
    return safeInvoke('overrule_orchestrator_task', { taskId, reason });
  }

  mercatusLoadConfig(): Promise<unknown> {
    return safeInvoke('mercatus_load_config');
  }

  mercatusSaveConfig(config: unknown): Promise<void> {
    return safeInvoke('mercatus_save_config', { config });
  }

  /** Vox Urbs (gamify town): workspace crate/file scan for the town layout. */
  workspaceTownScan(): Promise<TownScan> {
    return safeInvoke<TownScan>('workspace_town_scan');
  }

  /** Vox Urbs: CI fleet status tap (CASTRVM landmark). */
  harnessCiFleetStatus(): Promise<HarnessCiFleetDto> {
    return safeInvoke<HarnessCiFleetDto>('harness_ci_fleet_status');
  }

  /** Vox Urbs: VCS branch/PR status tap (PORTVS landmark). */
  vcsTownStatus(): Promise<HarnessVcsTownDto> {
    return safeInvoke<HarnessVcsTownDto>('vcs_town_status');
  }

  /** Vox Urbs: hopper queue tap (PORTVS ship count). */
  hopperList(): Promise<HarnessHopperItemDto[]> {
    return safeInvoke<HarnessHopperItemDto[]>('hopper_list');
  }

  /** Current per-category/per-source task-policy overrides (Settings panel). */
  getTaskPolicyOverrides(): Promise<TaskPolicyOverrides> {
    return safeInvoke<TaskPolicyOverrides>('get_task_policy_overrides');
  }

  /** `clutch`/`risk` merge onto the scope's existing entry — `undefined` leaves that axis unchanged. */
  setTaskPolicyOverride(
    scopeKind: 'category' | 'source',
    scopeKey: string,
    clutch?: string,
    risk?: string,
  ): Promise<void> {
    return safeInvoke<void>('set_task_policy_override', { scopeKind, scopeKey, clutch, risk });
  }

  clearTaskPolicyOverride(scopeKind: 'category' | 'source', scopeKey: string): Promise<void> {
    return safeInvoke<void>('clear_task_policy_override', { scopeKind, scopeKey });
  }
}

/** One category/source entry in {@link TaskPolicyOverrides}; either axis may be unset (inherit). */
export interface TaskPolicyEntry {
  clutch?: string;
  risk?: string;
}

/** Mirrors Rust `vox_orchestrator::config::TaskPolicyOverrides`. */
export interface TaskPolicyOverrides {
  category: Record<string, TaskPolicyEntry>;
  source: Record<string, TaskPolicyEntry>;
}

/** Mirrors Rust CI fleet status DTO consumed by the Vox Urbs harness taps. */
export interface HarnessCiFleetDto {
  runners: { name: string; busy: boolean; online: boolean }[];
  queued: number;
}

/** Mirrors Rust VCS town status DTO consumed by the Vox Urbs harness taps. */
export interface HarnessVcsTownDto {
  branches: { name: string; is_head: boolean; track: string }[];
  prs: { number: number; title: string; head_ref: string }[];
  prs_available: boolean;
}

/** Mirrors Rust hopper item DTO consumed by the Vox Urbs harness taps. */
export interface HarnessHopperItemDto {
  state: string;
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
  return safeInvoke<Suggestion[]>('discovery_suggest', { typed, limit });
}

export function discoveryHelp(actionId: string): Promise<ActionHelp | null> {
  return safeInvoke<ActionHelp | null>('discovery_help', { actionId });
}

export function discoveryRecord(
  actionId: string,
  used: boolean,
  nowMs: number,
  dwellMs: number,
): Promise<void> {
  return safeInvoke('discovery_record', { actionId, used, nowMs, dwellMs });
}

export function ptySpawn(tabId: string, cols: number, rows: number): Promise<void> {
  return safeInvoke('pty_spawn', { tabId, cols, rows });
}

export function ptyWrite(tabId: string, data: string): Promise<void> {
  return safeInvoke('pty_write', { tabId, data });
}

export function ptyClose(tabId: string): Promise<void> {
  return safeInvoke('pty_close', { tabId });
}

export const PTY_OUTPUT_EVENT = 'vox://pty-output';
export const PTY_EXIT_EVENT = 'vox://pty-exit';

export function listenPtyOutput(
  onChunk: (tabId: string, data: string) => void,
): Promise<UnlistenFn> {
  return safeListen<{ tab_id: string; data: string }>(PTY_OUTPUT_EVENT, (e) =>
    onChunk(e.payload.tab_id, e.payload.data),
  );
}

export function listenPtyExit(onExit: (tabId: string) => void): Promise<UnlistenFn> {
  return safeListen<{ tab_id: string }>(PTY_EXIT_EVENT, (e) => onExit(e.payload.tab_id));
}

// ---------------------------------------------------------------------------
// Policy enable/disable + edit transport wrappers.
// ---------------------------------------------------------------------------

export function policySetEnabled(id: string, enabled: boolean): Promise<void> {
  return safeInvoke('policy_set_enabled', { id, enabled });
}

export function policyEdit(id: string, title?: string, description?: string): Promise<void> {
  return safeInvoke('policy_edit', { id, title: title ?? null, description: description ?? null });
}

/** Send a free-form note to an agent's A2A inbox. Resolves to the message id. */
export function sendToAgent(agentId: string, body: string): Promise<string> {
  return safeInvoke<string>('send_to_agent', { agentId, body });
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
  return safeInvoke<ContextBudgetPayload>('get_context_budget', sessionId != null ? { sessionId } : {});
}

export type PlanNodeStatus =
  | 'pending'
  | 'queued'
  | 'in_progress'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'superseded'
  | 'blocked_on_approval';

export interface PlanNodeDto {
  node_id: string;
  description: string;
  status: PlanNodeStatus;
}

export function listPlanNodes(planSessionId: string, planVersion: number): Promise<PlanNodeDto[]> {
  return safeInvoke<PlanNodeDto[]>('list_plan_nodes', { planSessionId, planVersion });
}

export function updatePlanNode(
  planSessionId: string,
  planVersion: number,
  nodeId: string,
  description: string,
): Promise<void> {
  return safeInvoke<void>('update_plan_node', {
    input: { plan_session_id: planSessionId, plan_version: planVersion, node_id: nodeId, description },
  });
}

export function insertPlanNode(
  planSessionId: string,
  planVersion: number,
  nodeId: string,
  description: string,
  dependsOn: string[] = [],
): Promise<void> {
  return safeInvoke<void>('insert_plan_node', {
    input: {
      plan_session_id: planSessionId,
      plan_version: planVersion,
      node_id: nodeId,
      description,
      depends_on: dependsOn,
    },
  });
}

/** Flips every `blocked_on_approval` node in `planSessionId` back to
 *  `pending` — the `PlanPanel` footer's "Approve" button. Returns the
 *  number of nodes affected. */
export function approvePlanNodes(planSessionId: string): Promise<number> {
  return safeInvoke<number>('approve_plan_nodes', { planSessionId });
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
  return safeInvoke<ActivityRowDto[]>('activity_query', { filter });
}

export const ACTIVITY_APPENDED_EVENT = 'vox://activity-appended';

export function listenActivityAppended(onAppend: () => void): Promise<UnlistenFn> {
  return safeListen<void>(ACTIVITY_APPENDED_EVENT, () => onAppend());
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
  // `invoke_mcp_tool` (crates/vox-gui/src/commands/mcp.rs) is a Tauri command
  // returning `Result<Value, String>` — Tauri's IPC delivers this to JS as an
  // already-deserialized object `{ tool, is_error, result }`, never a JSON
  // string. `result` is the daemon's own `{ success, data, error? }` envelope
  // (already a parsed object too). Do NOT JSON.parse either — both are
  // objects. (Previously this called `JSON.parse(res)` on the object, which
  // JS coerces to the literal string "[object Object]" before JSON.parse
  // ever runs, throwing `SyntaxError: "[object Object]" is not valid JSON`.)
  const res = await safeInvoke<{ result: { success: boolean; data?: unknown; error?: string } }>(
    'invoke_mcp_tool',
    { tool: 'vox_feedback_list', args: {} },
  );
  const parsed = res.result;
  if (!parsed.success) {
    throw new Error(parsed.error || 'Failed to list feedback');
  }
  return normalizeFeedback(parsed.data);
}

export async function feedbackResolve(feedbackId: string, action: Record<string, unknown>): Promise<void> {
  const res = await safeInvoke<{ result: { success: boolean; error?: string } }>('invoke_mcp_tool', {
    tool: 'vox_resolve_feedback',
    args: { feedback_id: feedbackId, action }
  });
  const parsed = res.result;
  if (!parsed.success) {
    throw new Error(parsed.error || 'Failed to resolve feedback');
  }
}

export function listenFeedbackChanged(onChange: () => void): Promise<UnlistenFn> {
  return safeListen<any>(AGENT_EVENTS_EVENT, (e) => {
    const t = e?.payload?.kind?.type;
    if (t === 'feedback_requested' || t === 'feedback_resolved') onChange();
  });
}

export interface HopperTaskDto {
  item_id: string;
  intent: string;
  priority: number;
  state: string;
  task_id: number;
  session_id?: string | null;
  agent_id?: string | null;
  remote_node?: string | null;
}

/** List hopper task items (see `TasksView` / attention-inbox consumers). */
export function hopperList(): Promise<HopperTaskDto[]> {
  return safeInvoke<HopperTaskDto[]>('hopper_list');
}

/** Mark a hopper to-do done (terminal Done state; distinct from cancel). */
export function hopperMarkDone(itemId: string): Promise<HopperTaskDto> {
  return safeInvoke<HopperTaskDto>('hopper_mark_done', { itemId });
}

// ---------------------------------------------------------------------------
// Chat turn transport wrapper.
// ---------------------------------------------------------------------------

/** Mirrors Rust `ChatTurnInput` (`commands/chat_turn.rs`). Built exclusively by
 *  `lib/buildChatTurn.ts` so every composer control reaches the backend on both
 *  execution paths. */
export interface ChatTurnInput {
  session_id: string;
  content: string;
  /** Sync = terminal request/response. Background = orchestrator task with a
   *  correlated event stream. Plan = `vox_plan` with `require_approval: true`,
   *  the GUI's `/plan`. Set from the composer's send-mode toggle. */
  execution: 'sync' | 'background' | 'plan';
  model_override?: string | null;
  /** Composer "Run on" tier: local|mesh|cloud|auto. NOT `cognitive_profile`. */
  tier?: string | null;
  clutch?: string | null;
  risk?: string | null;
  context_files: string[];
  active_skill?: string | null;
  skill_exclusions: string[];
  /** Opt-in, non-blocking post-reply grounding/hallucination check (see
   *  `GroundingCheckToggle.tsx` and Rust `ChatTurnInput::grounding_check_enabled`). */
  grounding_check_enabled?: boolean | null;
  priority?: string | null;
  dry_run?: boolean | null;
  allow_duplicate?: boolean | null;
  /** Interaction mode from the composer (plan|act|verify); forwarded as the
   *  `mode` enqueue hint on the background path. */
  mode?: string | null;
  /** The real originating chat session, distinct from `session_id` above
   *  (which can be a synthetic background-session id). See Rust
   *  `ChatTurnInput::chat_session_id`. */
  chat_session_id?: string | null;
}

/** Mirrors Rust `ChatTurnDto` returned by `chat_turn`. On the background branch
 *  only `task_id` / `duplicate_of` are meaningful — that path persists no
 *  assistant row (`id` is 0, `content` empty). */
export interface ChatTurnDto {
  id: number;
  role: string;
  content: string;
  created_at: string;
  task_id: string | null;
  model_id?: string;
  latency_ms?: number;
  selection_reason?: string;
  /** True when the opt-in grounding check flagged this reply as low-confidence. */
  grounding_flagged?: boolean;
  /** Set (with a null `task_id`) when the daemon refused a near-duplicate. */
  duplicate_of?: string | null;
  /** Turn events derived from tool results this turn (e.g. a skill-activation
   *  chip) — empty on the background branch. See Rust `turn_event_for_result`. */
  events?: TurnEventDto[];
  /** Set on the `execution: 'plan'` branch only — the DAG `PlanPanel` should
   *  point at. */
  plan_session_id?: string | null;
  plan_version?: number | null;
}

/**
 * The single chat dispatch. `execution: 'sync'` runs the agent loop and returns
 * the persisted reply; `execution: 'background'` enqueues an orchestrator task
 * and returns its `task_id`. Throws on failure — see `lib/chatSend.ts` for the
 * higher-level `sendChatTurn` wrapper consumed by `App.tsx`.
 */
export function chatTurn(input: ChatTurnInput): Promise<ChatTurnDto> {
  return safeInvoke<ChatTurnDto>('chat_turn', { input });
}

// ---------------------------------------------------------------------------
// CodeRabbit sweep transport wrappers.
// ---------------------------------------------------------------------------

export function codeRabbitTokenPresent(): Promise<boolean> {
  return safeInvoke<boolean>('coderabbit_token_present');
}

/** Generic over the view's own `Report` shape — this hub has no opinion on it. */
export function codeRabbitReport<T = unknown>(): Promise<T> {
  return safeInvoke<T>('coderabbit_report');
}

export interface CodeRabbitSweepArgs {
  since: string;
  cap: number;
  rankWeights: string;
  top: number | null;
  fullRepo: boolean;
  [key: string]: unknown;
}

/** Generic over the view's own `Manifest` shape — this hub has no opinion on it. */
export function codeRabbitPlan<T = unknown>(args: CodeRabbitSweepArgs): Promise<T> {
  return safeInvoke<T>('coderabbit_plan', args);
}

export function codeRabbitRunAsync(args: CodeRabbitSweepArgs): Promise<void> {
  return safeInvoke('coderabbit_run_async', args);
}


