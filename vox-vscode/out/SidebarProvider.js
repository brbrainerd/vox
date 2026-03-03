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
exports.SidebarProvider = void 0;
const vscode = __importStar(require("vscode"));
class SidebarProvider {
    constructor(_extensionUri, _mcpConnection) {
        this._extensionUri = _extensionUri;
        this._mcpConnection = _mcpConnection;
    }
    resolveWebviewView(webviewView) {
        this._view = webviewView;
        webviewView.webview.options = {
            // Allow scripts in the webview
            enableScripts: true,
            localResourceRoots: [this._extensionUri],
        };
        webviewView.webview.html = this._getHtmlForWebview(webviewView.webview);
        webviewView.webview.onDidReceiveMessage(async (data) => {
            switch (data.type) {
                case "submitTask": {
                    if (!data.value)
                        return;
                    vscode.window.showInformationMessage(`Task sent to MCP: ${data.value}`);
                    const res = await this._mcpConnection.submitTask(data.value);
                    if (res) {
                        this._view?.webview.postMessage({ type: "taskResult", value: res });
                    }
                    break;
                }
                case "applyChanges": {
                    const { path, content } = data.value;
                    if (!path || !content) {
                        vscode.window.showErrorMessage("Apply changes failed: missing path or content.");
                        return;
                    }
                    try {
                        const uri = vscode.Uri.file(path);
                        const edit = new vscode.WorkspaceEdit();
                        // A simple full-file replace logic for demonstration
                        // In a real scenario we might do diff patching
                        edit.replace(uri, new vscode.Range(0, 0, 99999, 0), content);
                        const success = await vscode.workspace.applyEdit(edit);
                        if (success) {
                            await vscode.workspace.saveAll();
                            vscode.window.showInformationMessage(`Applied changes to ${path}`);
                        }
                        else {
                            vscode.window.showErrorMessage(`Failed to apply changes to ${path}`);
                        }
                    }
                    catch (e) {
                        vscode.window.showErrorMessage(`Error applying changes: ${e.message}`);
                    }
                    break;
                }
            }
        });
    }
    revive(panel) {
        this._view = panel;
    }
    _getHtmlForWebview(webview) {
        const scriptUri = webview.asWebviewUri(vscode.Uri.joinPath(this._extensionUri, "out", "webview.js"));
        // Use a nonce to only allow a specific script to be run.
        const nonce = getNonce();
        return `<!DOCTYPE html>
			<html lang="en">
			<head>
				<meta charset="UTF-8">
				<meta name="viewport" content="width=device-width, initial-scale=1.0">
				<script nonce="${nonce}">
          window.vscode = acquireVsCodeApi();
        </script>
			</head>
			<body>
        <div id="root"></div>
				<script nonce="${nonce}" src="${scriptUri}"></script>
			</body>
			</html>`;
    }
}
exports.SidebarProvider = SidebarProvider;
function getNonce() {
    let text = "";
    const possible = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    for (let i = 0; i < 32; i++) {
        text += possible.charAt(Math.floor(Math.random() * possible.length));
    }
    return text;
}
//# sourceMappingURL=SidebarProvider.js.map