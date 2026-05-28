import * as vscode from 'vscode';
import type { VoxMcpClient } from '../core/VoxMcpClient';

const DEFAULT_JOURNEY_ID = 'canonical_journey.v1.greenfield_vox_mens_devloop';

export function registerCanonicalJourneyChecklist(context: vscode.ExtensionContext, mcp: VoxMcpClient): void {
    context.subscriptions.push(
        vscode.commands.registerCommand('vox.journey.canonicalChecklist', async () => {
            if (!mcp.connected) {
                void vscode.window.showWarningMessage('Vox MCP is not connected yet; try again after the server attaches.');
                return;
            }
            if (!mcp.isToolAvailable('vox_journey_canonical_steps')) {
                void vscode.window.showWarningMessage(
                    'This workspace MCP server does not advertise `vox_journey_canonical_steps`. Update `vox` and ensure Codex (`VoxDb`) is attached.',
                );
                return;
            }
            const journey_id =
                (await vscode.window.showInputBox({
                    title: 'Canonical journey id',
                    value: DEFAULT_JOURNEY_ID,
                    ignoreFocusOut: true,
                })) ?? DEFAULT_JOURNEY_ID;
            if (!journey_id.trim()) {
                return;
            }
            const payload = await mcp.call<{ journey_id: string; steps: unknown[] }>('vox_journey_canonical_steps', {
                journey_id: journey_id.trim(),
            });
            if (!payload) {
                void vscode.window.showErrorMessage('MCP `vox_journey_canonical_steps` returned no data.');
                return;
            }
            const text = JSON.stringify(payload, null, 2);
            const doc = await vscode.workspace.openTextDocument({
                content: text,
                language: 'json',
            });
            await vscode.window.showTextDocument(doc, { preview: true });
        }),
    );
}
