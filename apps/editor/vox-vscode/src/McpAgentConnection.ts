import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import * as vscode from 'vscode';
// import { parseMcpToolResult, unwrapVoxToolEnvelope } from './core/mcpToolResult';

/** Standalone stdio MCP client (legacy / tests); prefer {@link VoxMcpClient} for the extension. */
export class McpAgentConnection {
    private client: Client;
    private seenEventIds = new Set<string>();
    private transport: StdioClientTransport;
    private _ws?: WebSocket;
    private _reconnectWsTimer?: NodeJS.Timeout;

    constructor(
        private outputChannel: vscode.OutputChannel,
        serverPath: string = 'vox',
        private onEvent?: (event: unknown) => void,
    ) {
        this.transport = new StdioClientTransport({
            command: serverPath,
            args: ['mcp'],
        });

        this.client = new Client(
            {
                name: 'vox-vscode-mcp-agent',
                version: '0.2.0',
            },
            {
                capabilities: {},
            },
        );

        this.client.fallbackNotificationHandler = async (notification) => {
            this.outputChannel.appendLine(`Received Notification: ${JSON.stringify(notification)}`);
            this.onEvent?.(notification);
        };
    }

    async connect(): Promise<void> {
        try {
            this.outputChannel.appendLine('Connecting to Vox MCP Server...');
            await this.client.connect(this.transport);
            this.outputChannel.appendLine('Connected to Vox MCP Server!');

            const toolsResponse = await this.client.listTools();
            this.outputChannel.appendLine(`Found ${toolsResponse.tools.length} MCP tools.`);

            if (this._ws) this._ws.close();
            this._connectWebSocket();
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : String(e);
            this.outputChannel.appendLine(`Failed to connect to MCP: ${msg}`);
        }
    }

    dispose(): void {
        clearTimeout(this._reconnectWsTimer);
        if (this._ws) this._ws.close();
        void this.client.close();
    }

    private _connectWebSocket(): void {
        if (!this.onEvent) return;
        const port = vscode.workspace.getConfiguration('vox').get<number>('mcp.httpPort') || 3921;
        const wsUrl = `ws://127.0.0.1:${port}/v1/ws`;
        
        try {
            this._ws = new globalThis.WebSocket(wsUrl);
            
            this._ws.onmessage = (event) => {
                try {
                    const data = JSON.parse(event.data);
                    if (data.msg_type === 'agent_event' && data.data) {
                        const ev = data.data;
                        const id = ev.id ?? `${ev.agent_id}-${ev.timestamp}-${ev.event_type ?? ev.type}`;
                        if (!this.seenEventIds.has(id)) {
                            this.seenEventIds.add(id);
                            if (this.onEvent) this.onEvent(ev);
                        }
                    }
                } catch {
                    // Ignore parse errors
                }
            };

            this._ws.onclose = () => {
                this._scheduleWsReconnect();
            };

            this._ws.onerror = () => {
                this._ws?.close();
            };
        } catch (_e) {
            this._scheduleWsReconnect();
        }
    }

    private _scheduleWsReconnect(): void {
        clearTimeout(this._reconnectWsTimer);
        this._reconnectWsTimer = setTimeout(() => {
            if (this._ws && this._ws.readyState !== globalThis.WebSocket.CLOSED) return;
            this._connectWebSocket();
        }, 5000);
    }



    async submitTask(task: string, accessMode: 'read' | 'write' = 'write') {
        try {
            const result = await this.client.callTool({
                name: 'vox_submit_task',
                arguments: {
                    description: task,
                    files: [{ path: '.', access: accessMode }],
                },
            });
            return result;
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : String(e);
            this.outputChannel.appendLine(`Error submitting task: ${msg}`);
            return null;
        }
    }
}
