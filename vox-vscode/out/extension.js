"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
const node_1 = require("vscode-languageclient/node");
const SidebarProvider_1 = require("./SidebarProvider");
const McpAgentConnection_1 = require("./McpAgentConnection");
const VisualEditorPanel_1 = require("./VisualEditorPanel");
let client;
function activate(context) {
    // Start LSP client
    const config = vscode.workspace.getConfiguration('vox');
    const lspEnabled = config.get('lsp.enabled', true);
    if (lspEnabled) {
        startLspClient(context, config);
    }
    // Register commands
    context.subscriptions.push(vscode.commands.registerCommand('vox.build', buildCurrentFile), vscode.commands.registerCommand('vox.run', runCurrentProject), vscode.commands.registerCommand('vox.restartLsp', () => restartLsp(context, config)), vscode.commands.registerCommand('vox.openVisualEditor', () => {
        VisualEditorPanel_1.VisualEditorPanel.createOrShow(context.extensionUri);
    }));
    const outputChannel = vscode.window.createOutputChannel('Vox MCP');
    const mcpConnection = new McpAgentConnection_1.McpAgentConnection(outputChannel, "vox", (event) => {
        // Forward polled events to our webview
        sidebarProvider._view?.webview.postMessage({
            type: 'agentEvent',
            value: event
        });
    });
    mcpConnection.connect();
    const sidebarProvider = new SidebarProvider_1.SidebarProvider(context.extensionUri, mcpConnection);
    context.subscriptions.push(vscode.window.registerWebviewViewProvider("vox-sidebar.chat", sidebarProvider));
    context.subscriptions.push(vscode.commands.registerCommand("vox.focusSidebar", () => {
        vscode.commands.executeCommand("vox-sidebar.chat.focus");
    }));
    // Status bar item
    const statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
    statusBar.text = '$(zap) Vox';
    statusBar.tooltip = 'Vox Language';
    statusBar.command = 'vox.build';
    statusBar.show();
    context.subscriptions.push(statusBar);
    console.log('Vox extension activated');
}
function startLspClient(context, config) {
    const customPath = config.get('lsp.serverPath', '');
    // Determine server command
    let command;
    let args;
    if (customPath) {
        command = customPath;
        args = [];
    }
    else {
        // Try to find vox-lsp in PATH, fall back to cargo run
        command = 'cargo';
        args = ['run', '-p', 'vox-lsp', '--release', '--'];
    }
    const serverOptions = {
        run: { command, args, transport: node_1.TransportKind.stdio },
        debug: { command, args, transport: node_1.TransportKind.stdio },
    };
    const clientOptions = {
        documentSelector: [{ scheme: 'file', language: 'vox' }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.vox'),
        },
        outputChannelName: 'Vox Language Server',
    };
    client = new node_1.LanguageClient('voxLanguageServer', 'Vox Language Server', serverOptions, clientOptions);
    client.start().then(() => console.log('Vox LSP started'), (err) => {
        console.warn('Vox LSP failed to start:', err);
        vscode.window.showWarningMessage('Vox Language Server failed to start. Run `cargo build -p vox-lsp --release` first.');
    });
    context.subscriptions.push({ dispose: () => client?.stop() });
}
async function restartLsp(context, config) {
    if (client) {
        await client.stop();
        client = undefined;
    }
    startLspClient(context, config);
    vscode.window.showInformationMessage('Vox Language Server restarted');
}
async function buildCurrentFile() {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== 'vox') {
        vscode.window.showWarningMessage('Open a .vox file to build');
        return;
    }
    const filePath = editor.document.uri.fsPath;
    const terminal = getOrCreateTerminal();
    terminal.show();
    terminal.sendText(`vox build "${filePath}" -o dist`);
}
async function runCurrentProject() {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    if (!workspaceFolder) {
        vscode.window.showWarningMessage('Open a workspace to run');
        return;
    }
    // Find the main .vox file
    const mainFiles = await vscode.workspace.findFiles('src/main.vox', null, 1);
    if (mainFiles.length === 0) {
        vscode.window.showWarningMessage('No src/main.vox found');
        return;
    }
    const terminal = getOrCreateTerminal();
    terminal.show();
    terminal.sendText(`vox run "${mainFiles[0].fsPath}"`);
}
function getOrCreateTerminal() {
    const existing = vscode.window.terminals.find((t) => t.name === 'Vox');
    if (existing) {
        return existing;
    }
    return vscode.window.createTerminal('Vox');
}
function deactivate() {
    return client?.stop();
}
//# sourceMappingURL=extension.js.map