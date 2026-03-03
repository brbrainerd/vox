import * as vscode from 'vscode';
import * as cp from 'child_process';
import { VoxMcpClient } from './VoxMcpClient';
import { ConfigManager } from './ConfigManager';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from 'vscode-languageclient/node';

export class LspClientManager {
    private _client?: LanguageClient;

    constructor(private readonly _context: vscode.ExtensionContext) {
        if (ConfigManager.lspEnabled) {
            this.start();
        }

        this._context.subscriptions.push(
            vscode.commands.registerCommand('vox.restartLsp', () => this.restart())
        );
    }

    start(): void {
        const customPath = ConfigManager.lspServerPath;
        let command: string;
        let args: string[];

        if (customPath) {
            command = customPath;
            args = [];
        } else {
            command = 'cargo';
            args = ['run', '-p', 'vox-lsp', '--release', '--'];
        }

        const serverOptions: ServerOptions = {
            run: { command, args, transport: TransportKind.stdio },
            debug: { command, args, transport: TransportKind.stdio },
        };

        const clientOptions: LanguageClientOptions = {
            documentSelector: [{ scheme: 'file', language: 'vox' }],
            synchronize: {
                fileEvents: vscode.workspace.createFileSystemWatcher('**/*.vox'),
            },
            outputChannelName: 'Vox Language Server',
        };

        this._client = new LanguageClient(
            'voxLanguageServer',
            'Vox Language Server',
            serverOptions,
            clientOptions,
        );

        this._client.start().then(
            () => console.log('Vox LSP started'),
            (err) => {
                console.warn('Vox LSP failed to start:', err);
                vscode.window.showWarningMessage(
                    'Vox Language Server failed to start. Run `cargo build -p vox-lsp --release` first.',
                );
            }
        );

        this._context.subscriptions.push({ dispose: () => this._client?.stop() });
    }

    async restart(): Promise<void> {
        if (this._client) {
            await this._client.stop();
            this._client = undefined;
        }
        if (ConfigManager.lspEnabled) {
            this.start();
            vscode.window.showInformationMessage('Vox Language Server restarted');
        }
    }

    dispose(): void {
        this._client?.stop();
    }
}

export function registerProjectCommands(context: vscode.ExtensionContext): void {
    context.subscriptions.push(
        vscode.commands.registerCommand('vox.build', buildCurrentFile),
        vscode.commands.registerCommand('vox.run', runCurrentProject)
    );
}

function getOrCreateTerminal(): vscode.Terminal {
    const existing = vscode.window.terminals.find((t) => t.name === 'Vox');
    if (existing) { return existing; }
    return vscode.window.createTerminal('Vox');
}

async function buildCurrentFile(): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== 'vox') {
        vscode.window.showWarningMessage('Open a .vox file to build');
        return;
    }
    const filePath = editor.document.uri.fsPath;
    const terminal = getOrCreateTerminal();
    terminal.show();
    terminal.sendText(`vox build "${filePath}" -o ${ConfigManager.buildOutputDir}`);
}

async function runCurrentProject(): Promise<void> {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    if (!workspaceFolder) {
        vscode.window.showWarningMessage('Open a workspace to run');
        return;
    }
    const mainFiles = await vscode.workspace.findFiles('src/main.vox', null, 1);
    if (mainFiles.length === 0) {
        vscode.window.showWarningMessage('No src/main.vox found');
        return;
    }
    const terminal = getOrCreateTerminal();
    terminal.show();
    terminal.sendText(`vox run "${mainFiles[0].fsPath}"`);
}
