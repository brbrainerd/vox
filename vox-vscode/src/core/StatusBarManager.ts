import * as vscode from 'vscode';
import * as cp from 'child_process';
import { VoxMcpClient } from '../core/VoxMcpClient';
import { ModelRegistryClient } from '../models/ModelRegistry';
import { ConfigManager } from './ConfigManager';

export class StatusBarManager {
    private _statusBar: vscode.StatusBarItem;
    private _timer?: NodeJS.Timeout;
    private _registry: ModelRegistryClient;

    constructor(
        private readonly _mcp: VoxMcpClient,
        context: vscode.ExtensionContext,
    ) {
        this._registry = new ModelRegistryClient(this._mcp);

        this._statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
        this._statusBar.text = '$(zap) Vox';
        this._statusBar.tooltip = 'Vox — loading provider status...';
        this._statusBar.command = 'vox.pickModel';
        this._statusBar.show();

        context.subscriptions.push(this._statusBar);
        context.subscriptions.push(
            vscode.commands.registerCommand('vox.refreshStatus', () => this.refresh())
        );

        this.start();
    }

    start(): void {
        this.refresh();
        this._timer = setInterval(() => this.refresh(), 60_000);
    }

    stop(): void {
        clearInterval(this._timer);
    }

    async refresh(): Promise<void> {
        let budget: any = null;
        if (this._mcp.connected) {
            budget = await this._mcp.budgetStatus();
        } else {
            budget = await fetchVoxStatusRaw();
        }

        const activeModelId = ConfigManager.activeModel;

        if (!budget?.providers) {
            this._statusBar.text = `$(zap) Vox: ${activeModelId.split('/').pop() ?? 'AI'}`;
            this._statusBar.tooltip = 'Vox MCP disconnected. Budget data unavailable.';
            this._statusBar.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
            return;
        }

        // Look up provider status matching the active model prefix
        const activeProvider = (budget.providers as any[]).find(
            p => activeModelId.startsWith(p.provider) || activeModelId.includes(p.provider)
        );

        const remainingStr = !activeProvider
            ? '–'
            : activeProvider.remaining === -1
            ? '∞'
            : `${activeProvider.remaining}/${activeProvider.daily_limit}`;

        const shortName = activeModelId.split('/').pop()?.replace('-preview', '').replace('-lite', 'L') ?? activeModelId;
        this._statusBar.text = `$(zap) Vox: ${shortName} (${remainingStr})`;

        if (activeProvider?.remaining === 0) {
            this._statusBar.backgroundColor = new vscode.ThemeColor('statusBarItem.errorBackground');
        } else {
            this._statusBar.backgroundColor = undefined;
        }

        // Rich tooltip
        const md = new vscode.MarkdownString();
        md.isTrusted = true;
        md.appendMarkdown('**Vox AI Provider Status**\n\n');
        for (const p of (budget.providers as any[])) {
            if (!p.configured) continue;
            const rem = p.remaining === -1 ? '∞' : `${p.remaining}/${p.daily_limit} remaining`;
            md.appendMarkdown(`• ${p.provider}: ${p.model} — ${rem}\n`);
        }
        if ((budget.cost_today_usd ?? 0) > 0) {
            md.appendMarkdown(`\nCost today: $${(budget.cost_today_usd as number).toFixed(4)}\n`);
        }
        md.appendMarkdown(`\n[Click to change model](command:vox.pickModel)`);
        this._statusBar.tooltip = md;
    }
}

/** Fallback: query `vox status --json` if MCP is not connected */
function fetchVoxStatusRaw(): Promise<unknown> {
    return new Promise(resolve => {
        cp.exec('vox status --json', { timeout: 5000 }, (err, stdout) => {
            if (err || !stdout.trim()) { resolve(null); return; }
            try { resolve(JSON.parse(stdout)); } catch { resolve(null); }
        });
    });
}
