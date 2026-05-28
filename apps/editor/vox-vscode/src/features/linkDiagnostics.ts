import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';

export function registerLinkDiagnostics(context: vscode.ExtensionContext) {
    const diagnosticCollection = vscode.languages.createDiagnosticCollection('vox-links');
    context.subscriptions.push(diagnosticCollection);

    const updateDiagnostics = async (document: vscode.TextDocument) => {
        if (document.languageId !== 'markdown') {
            return;
        }

        const diagnostics: vscode.Diagnostic[] = [];
        const text = document.getText();
        const linkRegex = /\[([^\]]+)\]\(([^)]+)\)/g;

        const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);
        if (!workspaceFolder) return;
        const repoRoot = workspaceFolder.uri.fsPath;
        const currentFilePath = document.uri.fsPath;
        const currentFileDir = path.dirname(currentFilePath);

        let match;
        while ((match = linkRegex.exec(text)) !== null) {
            const fullTarget = match[2];
            if (fullTarget.startsWith('http') || fullTarget.startsWith('#') || fullTarget.startsWith('mailto:')) {
                continue;
            }

            const parts = fullTarget.split('#');
            const targetPathStr = parts[0];
            const anchor = parts[1];

            if (!targetPathStr) continue;

            let targetPath: string;
            if (targetPathStr.startsWith('/')) {
                targetPath = path.join(repoRoot, targetPathStr.substring(1));
            } else {
                targetPath = path.join(currentFileDir, targetPathStr);
            }

            if (!fs.existsSync(targetPath)) {
                // Not found
                const startPos = document.positionAt(match.index);
                const endPos = document.positionAt(match.index + match[0].length);
                const range = new vscode.Range(startPos, endPos);
                
                const diagnostic = new vscode.Diagnostic(
                    range,
                    `Broken link: The file ${targetPathStr} does not exist.`,
                    vscode.DiagnosticSeverity.Error
                );
                diagnostics.push(diagnostic);
            } else if (anchor) {
                // Check anchor
                let foundAnchor = false;
                try {
                    const content = fs.readFileSync(targetPath, 'utf8');
                    // Fast naive check first
                    if (content.includes(`id="${anchor}"`) || content.includes(`name="${anchor}"`)) {
                        foundAnchor = true;
                    } else {
                        const lines = content.split('\n');
                        for (const line of lines) {
                            if (line.startsWith('#')) {
                                const headerText = line.replace(/^#+\s*/, '').trim();
                                const generated = headerText.toLowerCase().replace(/[^\w\s-]/g, '').replace(/\s+/g, '-');
                                if (generated === anchor) {
                                    foundAnchor = true;
                                    break;
                                }
                            }
                        }
                    }
                } catch {
                    // fall back
                }
                
                if (!foundAnchor) {
                    const startPos = document.positionAt(match.index);
                    const endPos = document.positionAt(match.index + match[0].length);
                    const range = new vscode.Range(startPos, endPos);
                    
                    const diagnostic = new vscode.Diagnostic(
                        range,
                        `Broken link: The anchor #${anchor} does not exist in ${targetPathStr}.`,
                        vscode.DiagnosticSeverity.Warning
                    );
                    diagnostics.push(diagnostic);
                }
            }
        }

        diagnosticCollection.set(document.uri, diagnostics);
    };

    vscode.workspace.onDidSaveTextDocument(updateDiagnostics, null, context.subscriptions);
    vscode.workspace.onDidOpenTextDocument(updateDiagnostics, null, context.subscriptions);
    
    // Initial check for all open markdown files
    vscode.workspace.textDocuments.forEach(doc => {
        if (doc.languageId === 'markdown') {
            updateDiagnostics(doc);
        }
    });
}
