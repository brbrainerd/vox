import * as vscode from "vscode";
import { McpAgentConnection } from "./McpAgentConnection";
export declare class SidebarProvider implements vscode.WebviewViewProvider {
    private readonly _extensionUri;
    private readonly _mcpConnection;
    _view?: vscode.WebviewView;
    constructor(_extensionUri: vscode.Uri, _mcpConnection: McpAgentConnection);
    resolveWebviewView(webviewView: vscode.WebviewView): void;
    revive(panel: vscode.WebviewView): void;
    private _getHtmlForWebview;
}
