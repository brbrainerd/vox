/**
 * VoxConfig — Typed accessor for shared toolchain configuration.
 *
 * SHARED SETTINGS (model, budget, data paths) must be read via the Orchestrator MCP
 * tool `vox_get_config`, NOT from VS Code workspace settings directly.
 *
 * This class provides typed helpers to call `vox_get_config` and cache the result.
 */

import type { VoxMcpClient } from './VoxMcpClient';

export interface VoxConfigResponse {
    model: string;
    daily_budget_usd: number;
    per_session_budget_usd: number;
    data_dir: string;
    model_dir: string;
    db_url: string | null;
}

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
            const config = await mcp.call<VoxConfigResponse>('vox_get_config', {});
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
        await mcp.call('vox_set_config', { key, value: String(value) });
        this._cache = null; // Invalidate cache
    }

    /** Invalidate the local config cache. */
    static invalidate(): void {
        this._cache = null;
    }
}
