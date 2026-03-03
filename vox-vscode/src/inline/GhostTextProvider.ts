import * as vscode from 'vscode';
import { VoxMcpClient } from '../core/VoxMcpClient';
import { ConfigManager } from '../core/ConfigManager';

interface CompletionCache {
    prefix: string;
    result: string;
}

const CACHE_MAX = 20;
const cache: CompletionCache[] = [];

function hashPrefix(prefix: string): string {
    let h = 0;
    for (let i = 0; i < prefix.length; i++) {
        h = (((h << 5) - h) + prefix.charCodeAt(i)) | 0;
    }
    return h.toString(36);
}

function getCached(prefix: string): string | undefined {
    const key = hashPrefix(prefix);
    return cache.find(c => hashPrefix(c.prefix) === key)?.result;
}

function putCache(prefix: string, result: string): void {
    if (cache.length >= CACHE_MAX) cache.shift();
    cache.push({ prefix, result });
}

export class GhostTextProvider implements vscode.InlineCompletionItemProvider {
    private _pending: AbortController | null = null;
    private _timer: NodeJS.Timeout | null = null;

    constructor(private readonly _mcp: VoxMcpClient) {}

    async provideInlineCompletionItems(
        document: vscode.TextDocument,
        position: vscode.Position,
        _context: vscode.InlineCompletionContext,
        token: vscode.CancellationToken,
    ): Promise<vscode.InlineCompletionList | null> {
        if (!ConfigManager.inlineAIEnabled) return null;

        // Debounce
        if (this._timer) clearTimeout(this._timer);

        return new Promise((resolve) => {
            this._timer = setTimeout(async () => {
                if (token.isCancellationRequested) { resolve(null); return; }

                const prefix = this._buildPrefix(document, position);
                const suffix = this._buildSuffix(document, position);

                // Check cache
                const cached = getCached(prefix);
                if (cached) {
                    resolve(new vscode.InlineCompletionList([
                        new vscode.InlineCompletionItem(cached, new vscode.Range(position, position))
                    ]));
                    return;
                }

                // Cancel previous pending
                this._pending?.abort();
                this._pending = new AbortController();

                if (!this._mcp.connected) { resolve(null); return; }

                const result = await this._mcp.generateCode({
                    type: 'completion',
                    prefix,
                    suffix,
                    language: document.languageId,
                    file: document.fileName,
                });

                if (token.isCancellationRequested || !result?.code) { resolve(null); return; }

                const code = result.code.trim();
                if (!code || code.length < 2) { resolve(null); return; }

                putCache(prefix, code);

                resolve(new vscode.InlineCompletionList([
                    new vscode.InlineCompletionItem(code, new vscode.Range(position, position))
                ]));
            }, ConfigManager.inlineAIDebounceMs);
        });
    }

    private _buildPrefix(document: vscode.TextDocument, position: vscode.Position): string {
        const startLine = Math.max(0, position.line - 20);
        const lines: string[] = [];
        for (let i = startLine; i <= position.line; i++) {
            const line = document.lineAt(i).text;
            lines.push(i === position.line ? line.substring(0, position.character) : line);
        }
        return lines.join('\n');
    }

    private _buildSuffix(document: vscode.TextDocument, position: vscode.Position): string {
        const endLine = Math.min(document.lineCount - 1, position.line + 5);
        const lines: string[] = [];
        for (let i = position.line; i <= endLine; i++) {
            const line = document.lineAt(i).text;
            lines.push(i === position.line ? line.substring(position.character) : line);
        }
        return lines.join('\n');
    }

    dispose(): void {
        if (this._timer) clearTimeout(this._timer);
        this._pending?.abort();
    }
}

export function registerGhostText(
    context: vscode.ExtensionContext,
    mcp: VoxMcpClient,
): vscode.Disposable {
    const provider = new GhostTextProvider(mcp);
    const languages = ConfigManager.inlineAILanguages;
    const selector: vscode.DocumentSelector = languages.includes('*')
        ? [{ pattern: '**/*' }]
        : languages.map(lang => ({ language: lang }));

    const registration = vscode.languages.registerInlineCompletionItemProvider(selector, provider);
    context.subscriptions.push(registration, { dispose: () => provider.dispose() });

    // Re-register when config changes
    context.subscriptions.push(ConfigManager.onChange(() => {
        // Extension reload handles re-registration
    }));

    return registration;
}
