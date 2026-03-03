import * as vscode from 'vscode';
import { AgentMode } from '../types';

export class ConfigManager {
    private static _config(): vscode.WorkspaceConfiguration {
        return vscode.workspace.getConfiguration('vox');
    }

    // LSP
    static get lspEnabled(): boolean { return this._config().get<boolean>('lsp.enabled', true); }
    static get lspServerPath(): string { return this._config().get<string>('lsp.serverPath', ''); }

    // Inline AI
    static get inlineAIEnabled(): boolean { return this._config().get<boolean>('inlineAI.enabled', true); }
    static get inlineAIDebounceMs(): number { return this._config().get<number>('inlineAI.debounceMs', 600); }
    static get inlineAIModel(): string { return this._config().get<string>('inlineAI.model', ''); }
    static get inlineAILanguages(): string[] {
        return this._config().get<string[]>('inlineAI.languages', ['*']);
    }

    // Composer
    static get composerAutoApply(): boolean { return this._config().get<boolean>('composer.autoApply', false); }

    // Agent
    static get agentDefaultMode(): AgentMode { return this._config().get<AgentMode>('agent.defaultMode', 'auto'); }

    // VCS
    static get vcsShowSnapshotBar(): boolean { return this._config().get<boolean>('vcs.showSnapshotBar', true); }

    // Gamify
    static get gamifyShowHud(): boolean { return this._config().get<boolean>('gamify.showHud', true); }

    // UI
    static get uiTheme(): string { return this._config().get<string>('ui.theme', 'auto'); }

    // Model
    static get activeModel(): string { return this._config().get<string>('ai.model', 'gemini-2.0-flash-lite'); }
    static async setActiveModel(model: string): Promise<void> {
        await this._config().update('ai.model', model, vscode.ConfigurationTarget.Global);
    }

    // BYOK
    static get byokGoogle(): string { return this._config().get<string>('models.byok.google', ''); }
    static get byokAnthropic(): string { return this._config().get<string>('models.byok.anthropic', ''); }
    static get byokOpenAI(): string { return this._config().get<string>('models.byok.openai', ''); }
    static get byokGroq(): string { return this._config().get<string>('models.byok.groq', ''); }
    static get byokTogether(): string { return this._config().get<string>('models.byok.together', ''); }

    static async setBYOK(provider: string, key: string): Promise<void> {
        await this._config().update(`models.byok.${provider}`, key, vscode.ConfigurationTarget.Global);
    }

    // Build
    static get buildOutputDir(): string { return this._config().get<string>('build.outputDir', 'dist'); }

    // Listen for changes
    static onChange(cb: () => void): vscode.Disposable {
        return vscode.workspace.onDidChangeConfiguration(e => {
            if (e.affectsConfiguration('vox')) cb();
        });
    }
}
