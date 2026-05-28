/**
 * VoxConfig — Typed accessor for shared toolchain configuration.
 *
 * SHARED SETTINGS (model, budget, data paths) must come from the Orchestrator MCP tool
 * `vox_config_get` (not VS Code workspace settings). The server still accepts the wire alias `vox_get_config`.
 */

import type { VoxMcpClient } from './VoxMcpClient';
import type { VoxConfigResponse } from '../types';

export type { VoxConfigResponse };

/**
 * Reads the shared VoxConfig from the Orchestrator (Rust → vox-config crate).
 * This is the single source of truth for all non-UX settings.
 *
 * Hierarchy (highest precedence first):
 *   CLI flags > ENV vars > Vox.toml > ~/.vox/config.toml > compiled defaults
 *
 * See: docs/agents/config-hierarchy.md
 */
export class VoxConfig {
    private static _cache: VoxConfigResponse | null = null;

    /** Fetch the full config from the Orchestrator. */
    static async load(mcp: VoxMcpClient): Promise<VoxConfigResponse | null> {
        try {
            const config = await mcp.configGet();
            if (config) {
                this._cache = config;
            }
            return config;
        } catch {
            return this._cache; // Return stale cache on error
        }
    }

    /** Fetch a single key from the Orchestrator config. */
    static async get(mcp: VoxMcpClient, key: keyof VoxConfigResponse): Promise<string | number | null | undefined> {
        const config = await this.load(mcp);
        return config?.[key];
    }

    /** Set a value in the Orchestrator config (writes to Vox.toml or ~/.vox/config.toml). */
    static async set(mcp: VoxMcpClient, key: string, value: string | number): Promise<void> {
        await mcp.configSet(key, String(value));
        this._cache = null; // Invalidate cache
    }

    /** Invalidate the local config cache. */
    static invalidate(): void {
        this._cache = null;
    }
}
