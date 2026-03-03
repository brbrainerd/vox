import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import * as vscode from "vscode";

export class McpAgentConnection {
    private client: Client;
    private seenEventIds = new Set<string>();
    private transport: StdioClientTransport;

    constructor(
        private outputChannel: vscode.OutputChannel,
        serverPath: string = "vox",
        private onEvent?: (event: any) => void
    ) {
        this.transport = new StdioClientTransport({
            command: serverPath,
            args: ["mcp"],
        });

        this.client = new Client(
            {
                name: "vox-vscode-client",
                version: "0.1.0"
            },
            {
                capabilities: {
                }
            }
        );

        this.client.fallbackNotificationHandler = async (notification) => {
            this.outputChannel.appendLine(`Received Notification: ${JSON.stringify(notification)}`);
            if (this.onEvent) {
                // Remove the "notifications/" prefix from the method if we want, or just pass the whole thing
                this.onEvent(notification);
            }
        };
    }

    async connect() {
        try {
            this.outputChannel.appendLine("Connecting to Vox MCP Server...");
            await this.client.connect(this.transport);
            this.outputChannel.appendLine("Connected to Vox MCP Server!");

            // Log tools available
            const toolsResponse = await this.client.listTools();
            this.outputChannel.appendLine(`Found ${toolsResponse.tools.length} MCP tools.`);

            // Poll for agent events
            setInterval(() => this.pollEvents(), 3000);
        } catch (e: any) {
            this.outputChannel.appendLine(`Failed to connect to MCP: ${e.message}`);
        }
    }

    private async pollEvents() {
        if (!this.onEvent) return;
        try {
            const [eventsResult, budgetResult] = await Promise.all([
                this.client.callTool({
                    name: "vox_poll_events",
                    arguments: { limit: 10 }
                }).catch(() => null),
                this.client.callTool({
                    name: "vox_budget_status",
                    arguments: {}
                }).catch(() => null)
            ]);

            if (eventsResult && eventsResult.content) {
                const content: any = eventsResult.content;
                if (content.length > 0) {
                    const text = content[0].type === 'text' ? content[0].text : "{}";
                const events = JSON.parse(text);
                if (Array.isArray(events)) {
                    for (const ev of events.reverse()) {
                        const id = (ev.id !== undefined && ev.id !== null) ? ev.id : `${ev.agent_id}-${ev.timestamp}-${ev.event_type || ev.type}`;
                        if (!this.seenEventIds.has(id)) {
                            this.seenEventIds.add(id);
                            this.onEvent(ev);
                        }
                    }
                }
                }
            }

            if (budgetResult && budgetResult.content) {
                const content: any = budgetResult.content;
                if (content.length > 0) {
                    const text = content[0].type === 'text' ? content[0].text : "{}";
                let budgetInfo = text;
                try {
                    const parsed = JSON.parse(text);
                    if (parsed.success !== false) budgetInfo = parsed;
                } catch (e) {}
                this.onEvent({ type: 'budget_status', data: budgetInfo });
                }
            }
        } catch (e) {
            // silent fail on poll
        }
    }

    async submitTask(task: string) {
        try {
            const result = await this.client.callTool({
                name: "vox_submit_task",
                arguments: {
                    description: task,
                    files: []
                }
            });
            return result;
        } catch (e: any) {
            this.outputChannel.appendLine(`Error submitting task: ${e.message}`);
            return null;
        }
    }
}
