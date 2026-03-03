// Thin model registry client — delegates ALL logic to the Rust MCP backend.
// The TS extension never hard-codes model data; it asks vox_list_models / vox_suggest_model instead.

import * as vscode from 'vscode';
import { VoxMcpClient } from '../core/VoxMcpClient';
import { ConfigManager } from '../core/ConfigManager';

export interface ModelSpec {
    id: string;
    provider: string;
    provider_type: 'google_direct' | 'open_router' | 'ollama';
    max_tokens: number;
    cost_per_1k: number;
    is_free: boolean;
    strengths: string[];
    display_name?: string;
    icon?: string;
}

export class ModelRegistryClient {
    private _cache: ModelSpec[] | null = null;
    private _cacheTs = 0;
    private static CACHE_TTL_MS = 5 * 60_000; // 5 min

    constructor(private readonly _mcp: VoxMcpClient) {}

    async listModels(): Promise<ModelSpec[]> {
        if (this._cache && Date.now() - this._cacheTs < ModelRegistryClient.CACHE_TTL_MS) {
            return this._cache;
        }
        const result = await this._mcp.call<ModelSpec[]>('vox_list_models', {});
        if (Array.isArray(result)) {
            this._cache = result;
            this._cacheTs = Date.now();
        }
        return this._cache ?? [];
    }

    async suggestModel(taskCategory: string): Promise<ModelSpec | null> {
        return this._mcp.call<ModelSpec>('vox_suggest_model', { task_category: taskCategory });
    }

    async getActive(): Promise<ModelSpec | null> {
        const id = ConfigManager.activeModel;
        const models = await this.listModels();
        return models.find(m => m.id === id) ?? null;
    }

    async setActive(id: string): Promise<void> {
        await ConfigManager.setActiveModel(id);
    }

    invalidateCache(): void {
        this._cache = null;
    }

    /** Build VS Code QuickPick items grouped by tier from Rust-provided model list */
    async buildQuickPickItems(): Promise<vscode.QuickPickItem[]> {
        const models = await this.listModels();
        const budget = await this._mcp.budgetStatus();
        const activeId = ConfigManager.activeModel;

        const groups: Array<{ sep: string; filter: (m: ModelSpec) => boolean }> = [
            { sep: '── FREE — Google AI Studio ────────', filter: m => m.is_free && m.provider_type === 'google_direct' },
            { sep: '── FREE — OpenRouter ───────────────', filter: m => m.is_free && m.provider_type === 'open_router' },
            { sep: '── LOCAL — Ollama ──────────────────', filter: m => m.provider_type === 'ollama' },
            { sep: '── BYOK / Paid ─────────────────────', filter: m => !m.is_free && m.provider_type !== 'ollama' },
        ];

        const items: vscode.QuickPickItem[] = [];
        for (const g of groups) {
            const groupModels = models.filter(g.filter);
            if (groupModels.length === 0) continue;
            items.push({ label: g.sep, kind: vscode.QuickPickItemKind.Separator });
            for (const m of groupModels) {
                const isActive = m.id === activeId;
                const providerInfo = budget?.providers?.find((p: any) => m.id.startsWith(p.provider));
                const remaining = (providerInfo as any)?.remaining;
                const ctxK = m.max_tokens >= 1_000_000 ? `${m.max_tokens / 1_000_000}M ctx` : `${(m.max_tokens / 1_000).toFixed(0)}K ctx`;
                const icon = m.provider_type === 'ollama' ? '$(server)' : m.is_free ? '$(sparkle)' : '$(key)';
                const costStr = m.cost_per_1k > 0 ? ` • $${(m.cost_per_1k * 1000).toFixed(2)}/1M tok` : ' • FREE';
                const usageStr = remaining !== undefined ? ` • ${remaining} remaining` : '';
                items.push({
                    label: `${icon}${isActive ? ' $(check)' : ''} ${m.display_name ?? m.id}`,
                    description: `${ctxK}${costStr}${usageStr}`,
                    detail: m.strengths.join(', '),
                });
            }
        }

        items.push({ label: '── Actions ─────────────────────────', kind: vscode.QuickPickItemKind.Separator });
        items.push({ label: '$(cloud-download) Pull Ollama Model...', description: 'Install a local model without internet after pull' });
        items.push({ label: '$(key) Set API Keys (BYOK)...', description: 'Anthropic, OpenAI, Groq, Together' });

        return items;
    }
}
