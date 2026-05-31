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
      return {
        exit_code: 64,
        stdout: '',
        stderr:
          `Operation "${name}" is MCP-only and not executable via the GUI sidecar. ` +
          'Use the MCP server integration path or add an IPC handler for this action.',
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
