import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

function diagnosticsEnabled(): boolean {
    return vscode.workspace.getConfiguration('vox').get<boolean>('webArtifacts.diagnosticsEnabled', true);
}

function refreshDocument(collection: vscode.DiagnosticCollection, document: vscode.TextDocument) {
    const base = path.basename(document.uri.fsPath);
    if (base !== 'routes.manifest.ts' && base !== 'vox-client.ts') {
        return;
    }
    if (!diagnosticsEnabled()) {
        collection.delete(document.uri);
        return;
    }
    const dir = path.dirname(document.uri.fsPath);
    const text = document.getText();
    const lines = text.split(/\r?\n/);
    const diags: vscode.Diagnostic[] = [];

    for (let i = 0; i < lines.length; i++) {
        const line = lines[i];
        if (line.includes('import type')) {
            continue;
        }
        const m = line.match(/^import\s+\{[^}]+\}\s+from\s+"(\.\/[^"]+)"/);
        if (!m) {
            continue;
        }
        const rel = m[1];
        const target = path.join(dir, rel);
        if (!fs.existsSync(target)) {
            const range = new vscode.Range(i, 0, i, Math.max(line.length, 1));
            diags.push(
                new vscode.Diagnostic(
                    range,
                    `Import target ${rel} not found next to this artifact (run \`vox build\` or fix the import path).`,
                    vscode.DiagnosticSeverity.Warning,
                ),
            );
        }
    }
    collection.set(document.uri, diags);
}

function refreshAllManifestDocs(collection: vscode.DiagnosticCollection) {
    for (const doc of vscode.workspace.textDocuments) {
        refreshDocument(collection, doc);
    }
}

/**
 * Warn when `routes.manifest.ts` / `vox-client.ts` import `./Foo.tsx` (etc.) that is missing on disk.
 */
export function registerWebArtifactImportDiagnostics(context: vscode.ExtensionContext) {
    const collection = vscode.languages.createDiagnosticCollection('vox-web-artifacts');
    context.subscriptions.push(collection);

    const refresh = (document: vscode.TextDocument) => refreshDocument(collection, document);

    refreshAllManifestDocs(collection);

    context.subscriptions.push(
        vscode.workspace.onDidOpenTextDocument(refresh),
        vscode.workspace.onDidChangeTextDocument((e) => refresh(e.document)),
        vscode.workspace.onDidChangeConfiguration((e) => {
            if (e.affectsConfiguration('vox.webArtifacts.diagnosticsEnabled')) {
                refreshAllManifestDocs(collection);
            }
        }),
        vscode.workspace.onDidSaveTextDocument((doc) => {
            const ext = path.extname(doc.uri.fsPath).toLowerCase();
            if (ext !== '.tsx' && ext !== '.ts' && ext !== '.css') {
                return;
            }
            refreshAllManifestDocs(collection);
        }),
    );

    const watcher = vscode.workspace.createFileSystemWatcher('**/{routes.manifest.ts,vox-client.ts}');
    context.subscriptions.push(watcher);
    watcher.onDidChange((uri) => {
        const doc = vscode.workspace.textDocuments.find((d) => d.uri.toString() === uri.toString());
        if (doc) {
            refresh(doc);
        }
    });
}
