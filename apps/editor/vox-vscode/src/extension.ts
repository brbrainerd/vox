import * as vscode from 'vscode';
import { VoxMcpClient } from './core/VoxMcpClient';
import { ConfigManager } from './core/ConfigManager';
import { LspClientManager, registerProjectCommands } from './core/LspClientManager';
import { StatusBarManager } from './core/StatusBarManager';
import { registerGhostText } from './inline/GhostTextProvider';
import { registerInlineEdit } from './inline/InlineEditController';
import { registerVcsCommands, UndoRedoManager } from './vcs/SnapshotProvider';
import { GamifyManager } from './gamify/GamifyManager';
import { AgentController } from './agents/AgentController';
import { registerCommandCatalogCommand } from './commands/commandCatalog';
import { registerCanonicalJourneyChecklist } from './commands/canonicalJourneyChecklist';
import { registerModelCommands } from './commands/model';

import { registerOratioSpeechCommands } from './speech/registerOratioSpeechCommands';
import { VisualEditorPanel } from './VisualEditorPanel';
import { registerLinkDiagnostics } from './features/linkDiagnostics';
import { registerWebArtifactImportDiagnostics } from './features/webArtifactDiagnostics';

export function activate(context: vscode.ExtensionContext) {
    const outputChannel = vscode.window.createOutputChannel('Vox');
    context.subscriptions.push(outputChannel);

    // ── Core MCP Connection ──────────────────────────────────────────────
    const mcp = new VoxMcpClient(outputChannel, ConfigManager.mcpServerPath);
    context.subscriptions.push({ dispose: () => mcp.dispose() });

    // Connect and start background services after VSCode is ready
    mcp.connect().then(() => {
        outputChannel.appendLine('[Vox] MCP connected. Starting background services.');
    });

    // ── LSP Client ───────────────────────────────────────────────────────
    const _lspManager = new LspClientManager(context);

    // ── Status Bar ───────────────────────────────────────────────────────
    const statusBar = new StatusBarManager(mcp, context);
    context.subscriptions.push({ dispose: () => statusBar.stop() });

    // ── Dashboard (Chat UI) ────────────────────────────────────────────────
    context.subscriptions.push(
        vscode.commands.registerCommand('vox.focusSidebar', () => {
            // Open the standalone dashboard instead of the legacy webview
            vscode.env.openExternal(vscode.Uri.parse('http://127.0.0.1:3921/dashboard'));
        }),
        vscode.commands.registerCommand('vox.openVisualEditor', () => {
            VisualEditorPanel.createOrShow(context.extensionUri);
        }),
    );

    // ── Inline AI ────────────────────────────────────────────────────────
    registerGhostText(context, mcp);
    registerInlineEdit(context, mcp);

    // ── Project Commands ─────────────────────────────────────────────────
    registerProjectCommands(context);
    
    // ── Diagnostics ──────────────────────────────────────────────────────
    registerLinkDiagnostics(context);
    registerWebArtifactImportDiagnostics(context);

    // ── Model Commands ───────────────────────────────────────────────────
    registerModelCommands(context, mcp);
    registerCanonicalJourneyChecklist(context, mcp);
    registerCommandCatalogCommand(context);
    registerOratioSpeechCommands(context, mcp);

    // ── VCS / Snapshot Tree (commands always registered; view visibility ↔ vox.vcs.showSnapshotBar)
    registerVcsCommands(context, mcp);
    const _undoRedo = new UndoRedoManager(mcp, context);

    // ── Agent Controller (live agent polling) ────────────────────────────
    const agentController = new AgentController(mcp, agents => {
        // Legacy sidebarProvider callback removed; the dashboard fetches this directly.
    });
    agentController.start();
    context.subscriptions.push({ dispose: () => agentController.stop() });

    context.subscriptions.push(
        vscode.commands.registerCommand('vox.debugMode', () => agentController.enableDebugMode()),
        vscode.commands.registerCommand('vox.rebalance', async () => {
            if (!mcp.isToolAvailable('vox_rebalance')) {
                void vscode.window.showInformationMessage(
                    'Rebalance requires `vox_rebalance` on the connected MCP server.',
                );
                return;
            }
            await agentController.rebalance();
        }),
        vscode.commands.registerCommand('vox.agent.spawn', async () => {
            const name = await vscode.window.showInputBox({
                title: 'Vox: Spawn agent',
                prompt: 'Name for the new agent',
                placeHolder: 'e.g. planner-1',
            });
            if (!name?.trim()) return;
            if (!mcp.isToolAvailable('vox_spawn_agent')) {
                void vscode.window.showWarningMessage(
                    'Server does not advertise `vox_spawn_agent`.',
                );
                return;
            }
            const dynamic =
                (await vscode.window.showQuickPick(['Standard', 'Dynamic (experimental)'], {
                    placeHolder: 'Agent kind',
                })) === 'Dynamic (experimental)';
            const ok = await mcp.spawnAgent(name.trim(), dynamic);
            if (ok != null) {
                void vscode.window.showInformationMessage(`Spawned agent “${name.trim()}”.`);
            } else {
                void vscode.window.showWarningMessage('Spawn failed — see Vox output channel.');
            }
        }),
    );

    // ── Gamification HUD ─────────────────────────────────────────────────
    const ludusStatusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 95);
    context.subscriptions.push(ludusStatusBar);
    const gamifyManager = new GamifyManager(
        mcp,
        state => {
            // Legacy sidebar callback removed; dashboard fetches this over WS.
        },
        ludusStatusBar,
    );
    gamifyManager.start();
    context.subscriptions.push({ dispose: () => gamifyManager.stop() });

    // ── Planning Mode ─────────────────────────────────────────────────────
    // vox.plan: generate a structured plan via native Rust and optionally open PLAN.md
    context.subscriptions.push(
        vscode.commands.registerCommand('vox.plan', async () => {
            const goal = await vscode.window.showInputBox({
                title: 'Vox: Planning Mode',
                prompt: 'Describe your goal. Vox will generate a structured task plan.',
                placeHolder: 'e.g. "Add authentication to the API with JWT tokens"',
            });
            if (!goal) return;

            const writeToDisk = await vscode.window.showQuickPick(
                ['Yes — write PLAN.md to workspace', 'No — show inline only'],
                { placeHolder: 'Write PLAN.md?' }
            );

            await vscode.window.withProgress(
                { location: vscode.ProgressLocation.Notification, title: '⟳ Vox is planning...', cancellable: false },
                async () => {
                    const result = await mcp.planGoal({
                        goal,
                        write_to_disk: writeToDisk?.startsWith('Yes') ?? false,
                        max_tasks: 40,
                    });

                    if (!result?.plan_md) {
                        vscode.window.showWarningMessage('Vox could not generate a plan. Check MCP connection.');
                        return;
                    }

                    // Open plan in editor
                    const planDoc = await vscode.workspace.openTextDocument({
                        language: 'markdown',
                        content: result.plan_md,
                    });
                    await vscode.window.showTextDocument(planDoc, vscode.ViewColumn.Beside);

                    if (result.written_to_disk) {
                        const planUri = vscode.Uri.joinPath(
                            vscode.workspace.workspaceFolders?.[0].uri ?? vscode.Uri.parse('.'),
                            'PLAN.md'
                        );
                        vscode.window.showInformationMessage('✓ Plan written to PLAN.md', 'Open').then(sel => {
                            if (sel === 'Open') vscode.window.showTextDocument(planUri);
                        });
                    }
                }
            );
        })
    );

    // Watch PLAN.md for external changes and notify sidebar
    const planWatcher = vscode.workspace.createFileSystemWatcher('**/PLAN.md');
    context.subscriptions.push(
        planWatcher,
        planWatcher.onDidChange(uri => {
            vscode.workspace.fs.readFile(uri).then(bytes => {
                // Legacy sidebar callback removed; dashboard watches via backend.
            });
        })
    );

    outputChannel.appendLine('[Vox] Extension activated successfully.');
}

export function deactivate(): void {
    // Disposables handle cleanup via context.subscriptions
}
