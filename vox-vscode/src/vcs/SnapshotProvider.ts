import * as vscode from 'vscode';
import { VoxMcpClient } from '../core/VoxMcpClient';
import type { Snapshot, OplogEntry } from '../types';

class SnapshotItem extends vscode.TreeItem {
    constructor(
        public readonly snapshot: Snapshot,
        public readonly collapsibleState: vscode.TreeItemCollapsibleState,
    ) {
        super(snapshot.message || `Snapshot ${snapshot.id}`, collapsibleState);
        const date = new Date(snapshot.timestamp * 1000);
        this.description = date.toLocaleTimeString();
        this.tooltip = `${snapshot.kind.toUpperCase()} snapshot\n${date.toLocaleString()}\n${snapshot.files_changed ?? '?'} files changed`;
        this.iconPath = this._getIcon(snapshot.kind);
        this.contextValue = 'snapshot';
        this.command = {
            command: 'vox.snapshotViewDiff',
            title: 'View Diff',
            arguments: [snapshot],
        };
    }

    private _getIcon(kind: 'code' | 'db' | 'full'): vscode.ThemeIcon {
        if (kind === 'db') return new vscode.ThemeIcon('database');
        if (kind === 'full') return new vscode.ThemeIcon('layers');
        return new vscode.ThemeIcon('git-commit');
    }
}

class OplogItem extends vscode.TreeItem {
    constructor(
        public readonly entry: OplogEntry,
    ) {
        super(entry.description || entry.op_type, vscode.TreeItemCollapsibleState.None);
        const date = new Date(entry.timestamp * 1000);
        this.description = date.toLocaleTimeString();
        this.tooltip = `${entry.op_type}\n${date.toLocaleString()}`;
        this.iconPath = entry.reversible
            ? new vscode.ThemeIcon('history')
            : new vscode.ThemeIcon('circle-slash');
        this.contextValue = 'oplog';
    }
}

type TreeNode = SnapshotItem | OplogItem | vscode.TreeItem;

export class SnapshotTreeProvider implements vscode.TreeDataProvider<TreeNode> {
    private _onDidChangeTreeData = new vscode.EventEmitter<void>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

    private _snapshots: Snapshot[] = [];
    private _oplogs: OplogEntry[] = [];

    constructor(private readonly _mcp: VoxMcpClient) {}

    async refresh(): Promise<void> {
        const [snapshots, oplogs] = await Promise.all([
            this._mcp.snapshotList(),
            this._mcp.oplog(),
        ]);
        this._snapshots = snapshots;
        this._oplogs = oplogs;
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: TreeNode): vscode.TreeItem {
        return element;
    }

    getChildren(element?: TreeNode): TreeNode[] {
        if (element) return [];

        const items: TreeNode[] = [];

        if (this._snapshots.length > 0) {
            const header = new vscode.TreeItem('SNAPSHOTS', vscode.TreeItemCollapsibleState.None);
            header.contextValue = 'header';
            items.push(header, ...this._snapshots.map(s =>
                new SnapshotItem(s, vscode.TreeItemCollapsibleState.None)
            ));
        }

        if (this._oplogs.length > 0) {
            const header = new vscode.TreeItem('OPERATION LOG', vscode.TreeItemCollapsibleState.None);
            header.contextValue = 'header';
            items.push(header, ...this._oplogs.slice(0, 20).map(e => new OplogItem(e)));
        }

        if (items.length === 0) {
            const empty = new vscode.TreeItem('No snapshots yet');
            empty.description = 'Complete a task to create one';
            items.push(empty);
        }

        return items;
    }
}

export class UndoRedoManager {
    private _statusBar: vscode.StatusBarItem;
    private _undoCount = 0;

    constructor(
        private readonly _mcp: VoxMcpClient,
        context: vscode.ExtensionContext,
    ) {
        this._statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 50);
        this._statusBar.command = 'vox.snapshotList';
        context.subscriptions.push(this._statusBar);

        context.subscriptions.push(
            vscode.commands.registerCommand('vox.undo', () => this.undo()),
            vscode.commands.registerCommand('vox.redo', () => this.redo()),
        );
    }

    async undo(): Promise<void> {
        const result = await this._mcp.undo();
        const desc = (result as { description?: string })?.description ?? 'Last operation';
        vscode.window.showInformationMessage(`↩ Undone: ${desc}`);
        this._undoCount = Math.max(0, this._undoCount - 1);
        this._updateStatus();
    }

    async redo(): Promise<void> {
        const result = await this._mcp.redo();
        const desc = (result as { description?: string })?.description ?? 'Operation';
        vscode.window.showInformationMessage(`↪ Redone: ${desc}`);
        this._undoCount++;
        this._updateStatus();
    }

    async snapshotBefore(message: string): Promise<void> {
        await this._mcp.submitTask(`[vcs] snapshot: ${message}`, []);
        this._undoCount++;
        this._updateStatus();
    }

    private _updateStatus(): void {
        if (this._undoCount > 0) {
            this._statusBar.text = `↩ ${this._undoCount} ops`;
            this._statusBar.tooltip = `${this._undoCount} undoable operations — click to view snapshots`;
            this._statusBar.show();
        } else {
            this._statusBar.hide();
        }
    }
}

export function registerVcsCommands(
    context: vscode.ExtensionContext,
    mcp: VoxMcpClient,
): void {
    const provider = new SnapshotTreeProvider(mcp);

    const treeView = vscode.window.createTreeView('vox-snapshots', {
        treeDataProvider: provider,
        showCollapseAll: true,
    });
    context.subscriptions.push(treeView);

    // Auto-refresh every 60s and on VCS events
    const refreshTimer = setInterval(() => provider.refresh(), 60_000);
    context.subscriptions.push({ dispose: () => clearInterval(refreshTimer) });

    context.subscriptions.push(
        vscode.commands.registerCommand('vox.snapshotRefresh', () => provider.refresh()),

        vscode.commands.registerCommand('vox.snapshotViewDiff', async (snapshot: Snapshot) => {
            const diff = await mcp.snapshotDiff(snapshot.id);
            if (!diff) { vscode.window.showWarningMessage('No diff available for this snapshot.'); return; }
            const doc = await vscode.workspace.openTextDocument({
                language: 'diff',
                content: typeof diff === 'string' ? diff : JSON.stringify(diff, null, 2),
            });
            await vscode.window.showTextDocument(doc, vscode.ViewColumn.Beside);
        }),

        vscode.commands.registerCommand('vox.snapshotRestore', async (snapshot: Snapshot) => {
            const confirm = await vscode.window.showWarningMessage(
                `Restore snapshot "${snapshot.message}"? This will revert your workspace.`,
                { modal: true },
                'Restore',
            );
            if (confirm !== 'Restore') return;
            await mcp.snapshotRestore(snapshot.id);
            vscode.window.showInformationMessage(`✓ Restored snapshot: ${snapshot.message}`);
            provider.refresh();
        }),

        vscode.commands.registerCommand('vox.snapshotList', () => {
            provider.refresh();
            treeView.reveal(undefined as unknown as vscode.TreeItem, { focus: true });
        }),
    );

    // Initial load
    provider.refresh();
}
