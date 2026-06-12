import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ActionManifest } from './types/actionManifest';

/** Tauri event name carrying the orchestrator status snapshot (see B1 daemon stream). */
export const ORCH_STATUS_EVENT = 'vox://orch-status';

/**
 * Subscribe to the pushed orchestrator-status event stream. The payload is the
 * same status object shape returned by `get_orchestrator_status` / daemon
 * `orch.status()` (fields like `agent_count`). Returns the `UnlistenFn` to call
 * on cleanup. Rejects if not running inside Tauri (caller should fall back to polling).
 */
export function listenOrchStatus(
  onStatus: (status: any) => void,
): Promise<UnlistenFn> {
  return listen<any>(ORCH_STATUS_EVENT, (event) => onStatus(event.payload));
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

export interface ExecuteOutput {
  exit_code: number;
  stdout: string;
  stderr: string;
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

  async getCatalog() {
    return invoke('get_command_catalog');
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

  async getRoutingSummaryLive() {
    return invoke('get_routing_summary_live');
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

/** Send a free-form note to an agent's A2A inbox. Resolves to the message id. */
export function sendToAgent(agentId: string, body: string): Promise<string> {
  return invoke<string>('send_to_agent', { agentId, body });
}
