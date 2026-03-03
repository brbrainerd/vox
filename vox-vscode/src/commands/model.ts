import * as vscode from 'vscode';
import { VoxMcpClient } from '../core/VoxMcpClient';
import { ModelRegistryClient } from '../models/ModelRegistry';
import { ConfigManager } from '../core/ConfigManager';

export function registerModelCommands(
    context: vscode.ExtensionContext,
    mcp: VoxMcpClient
): void {
    const registry = new ModelRegistryClient(mcp);

    context.subscriptions.push(
        vscode.commands.registerCommand('vox.pickModel', async () => {
            const items = await registry.buildQuickPickItems();
            const picked = await vscode.window.showQuickPick(items, {
                title: 'Vox: Select AI Model',
                placeHolder: 'Choose a model for Vox Chat, Composer, and Agents',
                matchOnDescription: true,
                matchOnDetail: true,
            });

            if (!picked || picked.kind === vscode.QuickPickItemKind.Separator) return;

            // Handle Actions
            if (picked.label.includes('Pull Ollama Model') || picked.label.includes('cloud-download')) {
                const modelName = await vscode.window.showInputBox({
                    prompt: 'Enter Ollama model name (e.g. llama3.2, mistral, codellama)',
                    placeHolder: 'model name',
                });
                if (modelName) {
                    const terminal = vscode.window.createTerminal('Vox Ollama Pull');
                    terminal.show();
                    terminal.sendText(`ollama pull ${modelName}`);
                    registry.invalidateCache();
                    setTimeout(() => registry.invalidateCache(), 15_000);
                }
                return;
            }

            if (picked.label.includes('Set API Keys') || picked.label.includes('BYOK')) {
                const provider = await vscode.window.showQuickPick(
                    [
                        { label: '$(key) Anthropic', id: 'anthropic' },
                        { label: '$(key) OpenAI', id: 'openai' },
                        { label: '$(key) Groq', id: 'groq' },
                        { label: '$(key) Together', id: 'together' },
                        { label: '$(key) OpenRouter', id: 'openrouter' },
                    ],
                    { placeHolder: 'Select provider to configure' }
                );
                if (!provider) return;
                const key = await vscode.window.showInputBox({
                    prompt: `Enter API Key for ${provider.id}`,
                    password: true,
                    ignoreFocusOut: true,
                });
                if (key) {
                    await ConfigManager.setBYOK(provider.id, key);
                    vscode.window.showInformationMessage(`✓ API Key for ${provider.id} saved.`);
                    registry.invalidateCache();
                }
                return;
            }

            // Model selection — extract ID from label
            const cleanLabel = picked.label
                .replace(/^\$\([a-z-]+\)\s+/, '')
                .replace(/\s*\$\(check\)\s*/, '')
                .trim();

            const models = await registry.listModels();
            const model = models.find(
                m => (m.display_name ?? m.id) === cleanLabel || m.id === cleanLabel
            );

            if (model) {
                await registry.setActive(model.id);
                // Also tell MCP to set the model preference persistently
                await mcp.preferenceSet('active_model', model.id);
                vscode.window.showInformationMessage(`Vox model → ${model.display_name ?? model.id}`);
            }
        })
    );
}
