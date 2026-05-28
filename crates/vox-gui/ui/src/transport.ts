import { invoke } from '@tauri-apps/api/core';

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

  async callTool(name: string, args: Record<string, any> = {}): Promise<ExecuteOutput> {
    const path = await this.resolvePath(name);
    const res = await invoke<ExecuteOutput>('execute_command', { path, args });
    return res;
  }

  async getMetadata(path: string[]): Promise<CommandMetadata | null> {
    return invoke('get_command_metadata', { path });
  }

  async getCatalog() {
    return invoke('get_command_catalog');
  }
}

export const voxTransport = new VoxTransport();
