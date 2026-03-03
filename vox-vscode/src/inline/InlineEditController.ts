// Thin InlineEditController — all context gathering and LLM calls delegated to vox_inline_edit (Rust).
// TypeScript handles only: (1) capturing the editor range/text, (2) rendering diff decorations,
// (3) Accept/Reject/Regenerate via CodeLens.

import * as vscode from 'vscode';
import { VoxMcpClient } from '../core/VoxMcpClient';

interface InlineEditResult {
    replacement: string;
    explanation: string;
    tokens: number;
    model_used: string;
}

interface PendingEdit {
    editorKey: string;
    originalText: string;
    newRange: vscode.Range;
    decorationType: vscode.TextEditorDecorationType;
    // Store params needed for Regenerate
    originalRange: vscode.Range;
    lastPrompt: string;
    lastLanguage: string;
}

const _pending = new Map<string, PendingEdit>();

class VoxCodeLensProvider implements vscode.CodeLensProvider {
    private _onChange = new vscode.EventEmitter<void>();
    readonly onDidChangeCodeLenses = this._onChange.event;
    private _lenses = new Map<string, vscode.Range>();

    set(key: string, range: vscode.Range): void { this._lenses.set(key, range); this._onChange.fire(); }
    clear(key: string): void { this._lenses.delete(key); this._onChange.fire(); }

    provideCodeLenses(document: vscode.TextDocument): vscode.CodeLens[] {
        const range = this._lenses.get(document.uri.toString());
        if (!range) return [];
        const r = new vscode.Range(range.start, range.start);
        return [
            new vscode.CodeLens(r, { title: '✓ Accept', command: 'vox.inlineEdit.accept', arguments: [document.uri.toString()] }),
            new vscode.CodeLens(r, { title: '✗ Reject', command: 'vox.inlineEdit.reject', arguments: [document.uri.toString()] }),
            new vscode.CodeLens(r, { title: '↻ Regenerate', command: 'vox.inlineEdit.regenerate', arguments: [document.uri.toString()] }),
        ];
    }
}

const codeLensProvider = new VoxCodeLensProvider();

export function registerInlineEdit(context: vscode.ExtensionContext, mcp: VoxMcpClient): void {
    context.subscriptions.push(
        vscode.languages.registerCodeLensProvider({ pattern: '**/*' }, codeLensProvider)
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('vox.inlineEdit', () => runEdit(mcp, 'edit')),
        vscode.commands.registerCommand('vox.inlineExplain', () => runEdit(mcp, 'explain')),
        vscode.commands.registerCommand('vox.inlineFix', () => runEdit(mcp, 'fix')),
        vscode.commands.registerCommand('vox.inlineEdit.accept', (key: string) => acceptEdit(key)),
        vscode.commands.registerCommand('vox.inlineEdit.reject', (key: string) => rejectEdit(key)),
        vscode.commands.registerCommand('vox.inlineEdit.regenerate', async (key: string) => {
            const p = _pending.get(key);
            if (!p) return;
            const prompt = p.lastPrompt;
            rejectEdit(key);
            await runEditWithPrompt(mcp, prompt, p.originalRange, p.lastLanguage);
        }),
        vscode.commands.registerCommand('vox.inlineEdit.escapeReject', () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) return;
            const key = editor.document.uri.toString();
            if (_pending.has(key)) rejectEdit(key);
            vscode.commands.executeCommand('setContext', 'vox.inlineEditActive', false);
        })
    );
}

async function runEdit(mcp: VoxMcpClient, variant: 'edit' | 'fix' | 'explain'): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (!editor) return;

    const selection = editor.selection;
    const range = selection.isEmpty ? editor.document.lineAt(selection.active.line).range : selection;
    const currentText = editor.document.getText(range);
    const language = editor.document.languageId;

    let prompt: string;
    if (variant === 'edit') {
        const input = await vscode.window.showInputBox({
            title: 'Vox: Inline Edit',
            prompt: 'Describe the change (e.g. "add error handling", "convert to async")',
            placeHolder: 'Your instruction...',
        });
        if (input === undefined) return;
        prompt = input;
    } else if (variant === 'fix') {
        prompt = 'Fix all bugs, type errors, and logic issues in this code.';
    } else {
        // explain — use generateCode explain path via MCP
        prompt = 'explain';
    }

    await runEditWithPrompt(mcp, prompt, range, language);
}

async function runEditWithPrompt(
    mcp: VoxMcpClient,
    prompt: string,
    range: vscode.Range,
    language: string,
): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (!editor) return;

    const key = editor.document.uri.toString();
    const doc = editor.document;

    // Recheck edit is still being targeted at this editor
    if (_pending.has(key)) rejectEdit(key);

    const currentText = doc.getText(range);
    const filePath = vscode.workspace.asRelativePath(doc.uri, false);

    // Gather surrounding context (up to 30 lines before/after)
    const contextBefore = doc.getText(new vscode.Range(
        new vscode.Position(Math.max(0, range.start.line - 30), 0),
        range.start,
    ));
    const contextAfter = doc.getText(new vscode.Range(
        range.end,
        new vscode.Position(Math.min(doc.lineCount - 1, range.end.line + 30), 9999),
    ));

    // Show loading indicator
    const loadDec = vscode.window.createTextEditorDecorationType({
        after: { contentText: ' ⟳ Vox…', color: new vscode.ThemeColor('editorCodeLens.foreground'), fontStyle: 'italic' },
    });
    editor.setDecorations(loadDec, [range]);
    vscode.commands.executeCommand('setContext', 'vox.inlineEditActive', true);

    // Handle explain separately
    if (prompt === 'explain') {
        const result = await mcp.generateCode({
            type: 'explain',
            selection: currentText,
            language,
            file: filePath,
            context: contextBefore + '\n[SELECTION]\n' + contextAfter,
        });
        loadDec.dispose();
        vscode.commands.executeCommand('setContext', 'vox.inlineEditActive', false);
        if (result?.explanation) {
            const doc = await vscode.workspace.openTextDocument({ language: 'markdown', content: `# Vox Explanation\n\n${result.explanation}` });
            vscode.window.showTextDocument(doc, vscode.ViewColumn.Beside);
        }
        return;
    }

    // Call native vox_inline_edit (Rust handles all context + LLM routing)
    const result = await mcp.call<InlineEditResult>('vox_inline_edit', {
        prompt,
        file: filePath,
        start_line: range.start.line + 1,
        end_line: range.end.line + 1,
        current_text: currentText,
        language,
        context_before: contextBefore,
        context_after: contextAfter,
    });

    loadDec.dispose();

    if (!result?.replacement) {
        vscode.commands.executeCommand('setContext', 'vox.inlineEditActive', false);
        vscode.window.showWarningMessage('Vox could not generate an edit. Try rephrasing your instruction.');
        return;
    }

    // Apply replacement to editor
    const edit = new vscode.WorkspaceEdit();
    edit.replace(doc.uri, range, result.replacement);
    await vscode.workspace.applyEdit(edit);

    // Compute new range after replacement
    const newLines = result.replacement.split('\n');
    const newEndLine = range.start.line + newLines.length - 1;
    const newEndChar = newLines.length === 1
        ? range.start.character + result.replacement.length
        : newLines[newLines.length - 1].length;
    const newRange = new vscode.Range(range.start.line, range.start.character, newEndLine, newEndChar);

    // Show diff decoration
    const diffDec = vscode.window.createTextEditorDecorationType({
        backgroundColor: new vscode.ThemeColor('diffEditor.insertedTextBackground'),
        border: '1px dashed',
        borderColor: new vscode.ThemeColor('editorInfo.foreground'),
    });
    editor.setDecorations(diffDec, [newRange]);

    _pending.set(key, {
        editorKey: key,
        originalText: currentText,
        newRange,
        decorationType: diffDec,
        originalRange: range,
        lastPrompt: prompt,
        lastLanguage: language,
    });
    codeLensProvider.set(key, newRange);
}

async function acceptEdit(key: string): Promise<void> {
    const p = _pending.get(key);
    if (!p) return;
    p.decorationType.dispose();
    codeLensProvider.clear(key);
    _pending.delete(key);
    vscode.commands.executeCommand('setContext', 'vox.inlineEditActive', false);
    await vscode.workspace.saveAll(false);
    vscode.window.showInformationMessage('✓ Vox edit accepted');
}

function rejectEdit(key: string): void {
    const p = _pending.get(key);
    if (!p) return;
    const editor = vscode.window.visibleTextEditors.find(e => e.document.uri.toString() === key);
    if (editor) {
        const edit = new vscode.WorkspaceEdit();
        edit.replace(editor.document.uri, p.newRange, p.originalText);
        vscode.workspace.applyEdit(edit);
    }
    p.decorationType.dispose();
    codeLensProvider.clear(key);
    _pending.delete(key);
    vscode.commands.executeCommand('setContext', 'vox.inlineEditActive', false);
}
