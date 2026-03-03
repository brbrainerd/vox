import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';

export class VisualEditorPanel {
    public static currentPanel: VisualEditorPanel | undefined;
    private readonly _panel: vscode.WebviewPanel;
    private readonly _extensionUri: vscode.Uri;
    private _disposables: vscode.Disposable[] = [];
    private _workspaceFolder: string | undefined;

    public static createOrShow(extensionUri: vscode.Uri) {
        const column = vscode.window.activeTextEditor
            ? vscode.window.activeTextEditor.viewColumn
            : undefined;

        if (VisualEditorPanel.currentPanel) {
            VisualEditorPanel.currentPanel._panel.reveal(column);
            return;
        }

        const panel = vscode.window.createWebviewPanel(
            'voxVisualEditor',
            'Vox Visual Editor',
            column || vscode.ViewColumn.One,
            {
                enableScripts: true,
                retainContextWhenHidden: true,
                localResourceRoots: [
                    vscode.Uri.joinPath(extensionUri, 'media'),
                    vscode.Uri.joinPath(extensionUri, 'out'),
                    ...(vscode.workspace.workspaceFolders ? vscode.workspace.workspaceFolders.map(f => f.uri) : [])
                ],
            }
        );

        VisualEditorPanel.currentPanel = new VisualEditorPanel(panel, extensionUri);
    }

    private constructor(panel: vscode.WebviewPanel, extensionUri: vscode.Uri) {
        this._panel = panel;
        this._extensionUri = extensionUri;

        if (vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders.length > 0) {
            this._workspaceFolder = vscode.workspace.workspaceFolders[0].uri.fsPath;
        }

        this._update();

        this._panel.onDidDispose(() => this.dispose(), null, this._disposables);

        // Update when active editor changes
        vscode.window.onDidChangeActiveTextEditor(
            (editor) => {
                if (editor && editor.document.languageId === 'vox') {
                    this._update();
                }
            },
            null,
            this._disposables
        );

        // Update when document is saved
        vscode.workspace.onDidSaveTextDocument(
            (document) => {
                if (document.languageId === 'vox') {
                    // Slight delay to allow build to complete if any background watcher is running
                    setTimeout(() => {
                        this._update();
                    }, 500);
                }
            },
            null,
            this._disposables
        );
    }

    public dispose() {
        VisualEditorPanel.currentPanel = undefined;
        this._panel.dispose();
        while (this._disposables.length) {
            const x = this._disposables.pop();
            if (x) {
                x.dispose();
            }
        }
    }

    private _update() {
        const webview = this._panel.webview;
        this._panel.title = "Vox Visual Editor";
        this._panel.webview.html = this._getHtmlForWebview(webview);
    }

    private _getHtmlForWebview(webview: vscode.Webview) {
        // Here we attempt to find built HTML/CSS or just point an iframe to localhost dev server.
        // For Vox, typically applications might be served locally.
        // We will default to an iframe to a local dev server, but fallback to rendering available index.html

        let htmlContent = "";

        // 1. Try to read dist/index.html if we statically generated it
        if (this._workspaceFolder) {
            const distHtmlPath = path.join(this._workspaceFolder, 'dist', 'index.html');
            if (fs.existsSync(distHtmlPath)) {
                try {
                    let rawHtml = fs.readFileSync(distHtmlPath, 'utf8');
                    // We might need to rewrite local paths to webview URIs
                    // But for now, we can just load it directly or inject a base tag
                    const distUri = webview.asWebviewUri(vscode.Uri.file(path.join(this._workspaceFolder, 'dist')));

                    // Simple replacement to make relative assets load via webview
                    htmlContent = rawHtml.replace(/(href|src)="(\.\/|\/)?([^"]+)"/g, (match, p1, p2, p3) => {
                        if (p3.startsWith('http') || p3.startsWith('data:')) return match;
                        return `${p1}="${distUri}/${p3}"`;
                    });

                    // Add an auto-refresh script
                    htmlContent = htmlContent.replace('</head>', `
                        <script>
                            window.addEventListener('message', event => {
                                if (event.data.type === 'refresh') {
                                    window.location.reload();
                                }
                            });
                        </script>
                        </head>
                    `);

                    return htmlContent;
                } catch(e) {
                    console.error("Error reading dist/index.html", e);
                }
            }
        }

        // 2. Default fallback: iframe to a typical dev server port (e.g. 3000, 5173, 8080)
        // This handles cases where vox is running via Vite/Next or our own python backend
        return `<!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Vox Live Render</title>
            <style>
                body, html { margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden; background: #fff; }
                iframe { width: 100%; height: 100%; border: none; }
                .overlay {
                    position: absolute;
                    top: 10px; right: 10px;
                    background: rgba(0,0,0,0.7); color: white;
                    padding: 5px 10px; border-radius: 4px;
                    font-family: sans-serif; font-size: 12px;
                    z-index: 1000;
                    pointer-events: none;
                }
            </style>
        </head>
        <body>
            <div class="overlay">Live View (Waiting for build...)</div>
            <iframe id="preview" src="http://localhost:3000" onerror="this.src='http://localhost:5173'"></iframe>
            <script>
                // Add message listener for soft reloads
                window.addEventListener('message', event => {
                    if (event.data.type === 'refresh') {
                        document.getElementById('preview').src = document.getElementById('preview').src;
                    }
                });
            </script>
        </body>
        </html>`;
    }
}
