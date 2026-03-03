// Thin WorkspaceContextEngine — the extension no longer does @mention resolution or file scanning.
// It just: (1) passes active editor metadata to the MCP when needed, (2) provides a helper to
// attach the active file/selection as explicit context for chat/inline requests.

import * as vscode from 'vscode';

export interface ActiveEditorContext {
    /** Workspace-relative file path, empty string if no editor open */
    filePath: string;
    /** Current cursor line (1-indexed) */
    line: number;
    /** Selected text if any */
    selectedText: string;
    /** Language ID of the active document */
    languageId: string;
    /** All urgent diagnostic messages in the active file */
    diagnostics: Array<{ severity: 'error' | 'warning'; line: number; message: string; source?: string }>;
}

export class WorkspaceContextEngine {
    /** Snapshot of what is visible right now in the editor for attaching to requests */
    getActiveEditorContext(): ActiveEditorContext {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            return { filePath: '', line: 0, selectedText: '', languageId: '', diagnostics: [] };
        }

        const doc = editor.document;
        const relativePath = vscode.workspace.asRelativePath(doc.uri, false);
        const line = editor.selection.active.line + 1;
        const selectedText = editor.selection.isEmpty ? '' : doc.getText(editor.selection);

        const diags = vscode.languages.getDiagnostics(doc.uri)
            .filter(d => d.severity <= vscode.DiagnosticSeverity.Warning)
            .slice(0, 20)
            .map(d => ({
                severity: d.severity === vscode.DiagnosticSeverity.Error ? 'error' as const : 'warning' as const,
                line: d.range.start.line + 1,
                message: d.message,
                source: d.source,
            }));

        return { filePath: relativePath, line, selectedText, languageId: doc.languageId, diagnostics: diags };
    }

    /** List currently open editor paths (workspace-relative), capped at 20  */
    getOpenFilePaths(): string[] {
        return vscode.window.visibleTextEditors
            .map(e => vscode.workspace.asRelativePath(e.document.uri, false))
            .filter(p => !p.startsWith('extension-output-'))
            .slice(0, 20);
    }
}
