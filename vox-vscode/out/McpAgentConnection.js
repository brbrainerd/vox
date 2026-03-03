"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.McpAgentConnection = void 0;
const index_js_1 = require("@modelcontextprotocol/sdk/client/index.js");
const stdio_js_1 = require("@modelcontextprotocol/sdk/client/stdio.js");
class McpAgentConnection {
    constructor(outputChannel, serverPath = "vox", onEvent) {
        this.outputChannel = outputChannel;
        this.onEvent = onEvent;
        this.seenEventIds = new Set();
        this.transport = new stdio_js_1.StdioClientTransport({
            command: serverPath,
            args: ["opencode", "start", "--port", "0"],
        });
        this.client = new index_js_1.Client({
            name: "vox-vscode-client",
            version: "0.1.0"
        }, {
            capabilities: {}
        });
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
        }
        catch (e) {
            this.outputChannel.appendLine(`Failed to connect to MCP: ${e.message}`);
        }
    }
    async pollEvents() {
        if (!this.onEvent)
            return;
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
                const content = eventsResult.content;
                if (content.length > 0) {
                    const text = content[0].type === 'text' ? content[0].text : "{}";
                    const events = JSON.parse(text);
                    if (Array.isArray(events)) {
                        for (const ev of events.reverse()) {
                            const id = ev.id || `${ev.agent_id}-${ev.timestamp}-${ev.type}`;
                            if (!this.seenEventIds.has(id)) {
                                this.seenEventIds.add(id);
                                this.onEvent(ev);
                            }
                        }
                    }
                }
            }
            if (budgetResult && budgetResult.content) {
                const content = budgetResult.content;
                if (content.length > 0) {
                    const text = content[0].type === 'text' ? content[0].text : "{}";
                    let budgetInfo = text;
                    try {
                        const parsed = JSON.parse(text);
                        if (parsed.success !== false)
                            budgetInfo = parsed;
                    }
                    catch (e) { }
                    this.onEvent({ type: 'budget_status', data: budgetInfo });
                }
            }
        }
        catch (e) {
            // silent fail on poll
        }
    }
    async submitTask(task) {
        try {
            const result = await this.client.callTool({
                name: "vox_submit_task",
                arguments: {
                    description: task,
                    files: []
                }
            });
            return result;
        }
        catch (e) {
            this.outputChannel.appendLine(`Error submitting task: ${e.message}`);
            return null;
        }
    }
}
exports.McpAgentConnection = McpAgentConnection;
//# sourceMappingURL=McpAgentConnection.js.map