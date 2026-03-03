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
import { registerModelCommands } from './commands/model';
import { SidebarProvider } from './SidebarProvider';

export function activate(context: vscode.ExtensionContext) {
    const outputChannel = vscode.window.createOutputChannel('Vox');
    context.subscriptions.push(outputChannel);

    // ── Core MCP Connection ──────────────────────────────────────────────
    const mcp = new VoxMcpClient(outputChannel);
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

    // ── Sidebar (Chat UI) ────────────────────────────────────────────────
    const sidebarProvider = new SidebarProvider(context.extensionUri, mcp);
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider('vox-sidebar.chat', sidebarProvider)
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('vox.focusSidebar', () => {
            vscode.commands.executeCommand('vox-sidebar.chat.focus');
        })
    );

    // ── Inline AI ────────────────────────────────────────────────────────
    registerGhostText(context, mcp);
    registerInlineEdit(context, mcp);

    // ── Project Commands ─────────────────────────────────────────────────
    registerProjectCommands(context);

    // ── Model Commands ───────────────────────────────────────────────────
    registerModelCommands(context, mcp);

    // ── VCS / Snapshot Tree ──────────────────────────────────────────────
    if (ConfigManager.vcsShowSnapshotBar) {
        registerVcsCommands(context, mcp);
        const _undoRedo = new UndoRedoManager(mcp, context);
    }

    // ── Agent Controller (live agent polling) ────────────────────────────
    const agentController = new AgentController(mcp, agents => {
        sidebarProvider.postMessage({ type: 'agentsUpdate', value: agents });
    });
    agentController.start();
    context.subscriptions.push({ dispose: () => agentController.stop() });

    context.subscriptions.push(
        vscode.commands.registerCommand('vox.debugMode', () => agentController.enableDebugMode()),
        vscode.commands.registerCommand('vox.rebalance', () => agentController.rebalance()),
    );

    // ── Gamification HUD ─────────────────────────────────────────────────
    const gamifyManager = new GamifyManager(mcp, state => {
        sidebarProvider.postMessage({ type: 'gamifyUpdate', value: state });
    });
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
                    const result = await mcp.call<{ plan_md: string; tasks: unknown[]; written_to_disk: boolean }>(
                        'vox_plan',
                        {
                            goal,
                            write_to_disk: writeToDisk?.startsWith('Yes'),
                            max_tasks: 40,
                        }
                    );

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
                const content = Buffer.from(bytes).toString('utf8');
                sidebarProvider.postMessage({ type: 'planUpdate', value: content });
            });
        })
    );

    outputChannel.appendLine('[Vox] Extension activated successfully.');
}

export function deactivate(): void {
    // Disposables handle cleanup via context.subscriptions
}
