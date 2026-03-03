import * as vscode from 'vscode';
export declare class VisualEditorPanel {
    static currentPanel: VisualEditorPanel | undefined;
    private readonly _panel;
    private readonly _extensionUri;
    private _disposables;
    private _workspaceFolder;
    static createOrShow(extensionUri: vscode.Uri): void;
    private constructor();
    dispose(): void;
    private _update;
    private _getHtmlForWebview;
}
