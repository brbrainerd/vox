import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import * as vscode from 'vscode';
import type {
    AgentEvent,
    Snapshot,
    OplogEntry,
    BudgetStatus,
    GamifyState,
    SkillInfo,
} from '../types';

interface McpResult {
    content: Array<{ type: string; text: string }>;
}

function parseResult(result: unknown): unknown {
    const r = result as McpResult | null;
    if (!r || !r.content || r.content.length === 0) return null;
    const text = r.content[0]?.type === 'text' ? r.content[0].text : '{}';
    try { return JSON.parse(text); } catch { return text; }
}

export class VoxMcpClient {
    private client: Client;
    private transport: StdioClientTransport;
    private _connected = false;
    private _reconnectDelay = 1000;
    private _reconnectTimer?: NodeJS.Timeout;
    public outputChannel: vscode.OutputChannel;

    constructor(outputChannel: vscode.OutputChannel, serverPath = 'vox') {
        this.outputChannel = outputChannel;
        this.transport = new StdioClientTransport({ command: serverPath, args: ['mcp'] });
        this.client = new Client(
            { name: 'vox-vscode-client', version: '0.2.0' },
            { capabilities: {} }
        );
        this.client.fallbackNotificationHandler = async (notification) => {
            this.outputChannel.appendLine(`[MCP Notification] ${JSON.stringify(notification)}`);
        };
    }

    get connected(): boolean { return this._connected; }

    async connect(): Promise<void> {
        try {
            this.outputChannel.appendLine('[Vox MCP] Connecting...');
            await this.client.connect(this.transport);
            this._connected = true;
            this._reconnectDelay = 1000;
            const tools = await this.client.listTools();
            this.outputChannel.appendLine(`[Vox MCP] Connected. ${tools.tools.length} tools available.`);
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : String(e);
            this.outputChannel.appendLine(`[Vox MCP] Failed to connect: ${msg}`);
            this._connected = false;
            this._scheduleReconnect();
        }
    }

    private _scheduleReconnect(): void {
        clearTimeout(this._reconnectTimer);
        this._reconnectTimer = setTimeout(async () => {
            this.outputChannel.appendLine(`[Vox MCP] Reconnecting (delay: ${this._reconnectDelay}ms)...`);
            // Re-create transport for fresh connection
            this.transport = new StdioClientTransport({ command: 'vox', args: ['mcp'] });
            this.client = new Client(
                { name: 'vox-vscode-client', version: '0.2.0' },
                { capabilities: {} }
            );
            await this.connect();
            this._reconnectDelay = Math.min(this._reconnectDelay * 2, 30000);
        }, this._reconnectDelay);
    }

    /** Public accessor so thin-client wrappers can call arbitrary MCP tools without boilerplate. */
    async call<T>(name: string, args: Record<string, unknown>): Promise<T | null> {
        if (!this._connected) return null;
        try {
            const result = await this.client.callTool({ name, arguments: args });
            return parseResult(result) as T;
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : String(e);
            this.outputChannel.appendLine(`[Vox MCP] Tool error [${name}]: ${msg}`);
            if (msg.includes('Connection') || msg.includes('ENOENT') || msg.includes('closed')) {
                this._connected = false;
                this._scheduleReconnect();
            }
            return null;
        }
    }

    // ── Task & Orchestration ──────────────────────────────────────────────────
    async submitTask(description: string, files: string[] = [], mode?: string): Promise<unknown> {
        return this.call('vox_submit_task', { description, files, ...(mode ? { mode } : {}) });
    }
    async taskStatus(taskId: string): Promise<unknown> {
        return this.call('vox_task_status', { task_id: taskId });
    }
    async completeTask(taskId: string): Promise<unknown> {
        return this.call('vox_complete_task', { task_id: taskId });
    }
    async cancelTask(taskId: string): Promise<unknown> {
        return this.call('vox_cancel_task', { task_id: taskId });
    }
    async orchestratorStatus(): Promise<GamifyState | null> {
        return this.call<GamifyState>('vox_orchestrator_status', {});
    }
    async rebalance(): Promise<unknown> {
        return this.call('vox_rebalance', {});
    }
    async pollEvents(limit = 20): Promise<AgentEvent[]> {
        const result = await this.call<AgentEvent[]>('vox_poll_events', { limit });
        return Array.isArray(result) ? result : [];
    }

    // ── Code Generation ───────────────────────────────────────────────────────
    async generateCode(payload: {
        type: 'completion' | 'edit' | 'explain' | 'fix';
        prompt?: string;
        prefix?: string;
        suffix?: string;
        selection?: string;
        language?: string;
        file?: string;
        context?: string;
    }): Promise<{ code?: string; explanation?: string; tokens?: number } | null> {
        return this.call('vox_generate_code', payload as Record<string, unknown>);
    }

    // ── VCS & Snapshots ───────────────────────────────────────────────────────
    async snapshotList(): Promise<Snapshot[]> {
        const result = await this.call<Snapshot[]>('vox_snapshot_list', {});
        return Array.isArray(result) ? result : [];
    }
    async snapshotDiff(snapshotId: string): Promise<string | null> {
        return this.call<string>('vox_snapshot_diff', { snapshot_id: snapshotId });
    }
    async snapshotRestore(snapshotId: string): Promise<unknown> {
        return this.call('vox_snapshot_restore', { snapshot_id: snapshotId });
    }
    async oplog(): Promise<OplogEntry[]> {
        const result = await this.call<OplogEntry[]>('vox_oplog', {});
        return Array.isArray(result) ? result : [];
    }
    async undo(): Promise<{ description?: string } | null> {
        return this.call('vox_undo', {});
    }
    async redo(): Promise<{ description?: string } | null> {
        return this.call('vox_redo', {});
    }
    async vcsStatus(): Promise<unknown> {
        return this.call('vox_vcs_status', {});
    }

    // ── Compiler & Tests ──────────────────────────────────────────────────────
    async validateFile(path: string): Promise<unknown> {
        return this.call('vox_validate_file', { path });
    }
    async runTests(crate?: string): Promise<unknown> {
        return this.call('vox_run_tests', crate ? { crate } : {});
    }
    async checkWorkspace(): Promise<unknown> {
        return this.call('vox_check_workspace', {});
    }

    // ── Budget & Preferences ──────────────────────────────────────────────────
    async budgetStatus(): Promise<BudgetStatus | null> {
        return this.call<BudgetStatus>('vox_budget_status', {});
    }
    async preferenceGet(key: string): Promise<unknown> {
        return this.call('vox_preference_get', { key });
    }
    async preferenceSet(key: string, value: unknown): Promise<unknown> {
        return this.call('vox_preference_set', { key, value: JSON.stringify(value) });
    }

    // ── Memory & Knowledge ────────────────────────────────────────────────────
    async memoryStore(key: string, value: string): Promise<unknown> {
        return this.call('vox_memory_store', { key, value });
    }
    async memoryRecall(key: string): Promise<{ value?: string } | null> {
        return this.call('vox_memory_recall', { key });
    }
    async memorySearch(query: string): Promise<unknown[]> {
        const result = await this.call<unknown[]>('vox_memory_search', { query });
        return Array.isArray(result) ? result : [];
    }
    async knowledgeQuery(query: string): Promise<unknown> {
        return this.call('vox_knowledge_query', { query });
    }

    // ── Skills ────────────────────────────────────────────────────────────────
    async skillList(): Promise<SkillInfo[]> {
        const result = await this.call<SkillInfo[]>('vox_skill_list', {});
        return Array.isArray(result) ? result : [];
    }
    async skillSearch(query: string): Promise<SkillInfo[]> {
        const result = await this.call<SkillInfo[]>('vox_skill_search', { query });
        return Array.isArray(result) ? result : [];
    }
    async skillInstall(id: string): Promise<unknown> {
        return this.call('vox_skill_install', { skill_id: id });
    }
    async skillUninstall(id: string): Promise<unknown> {
        return this.call('vox_skill_uninstall', { skill_id: id });
    }
    async skillInfo(id: string): Promise<SkillInfo | null> {
        return this.call<SkillInfo>('vox_skill_info', { skill_id: id });
    }

    // ── Agent A2A ─────────────────────────────────────────────────────────────
    async askAgent(agentId: string, question: string): Promise<unknown> {
        return this.call('vox_ask_agent', { agent_id: agentId, question });
    }
    async a2aSend(toAgent: string, message: string): Promise<unknown> {
        return this.call('vox_a2a_send', { to_agent: toAgent, message });
    }
    async a2aInbox(): Promise<unknown[]> {
        const result = await this.call<unknown[]>('vox_a2a_inbox', {});
        return Array.isArray(result) ? result : [];
    }

    // ── Session ───────────────────────────────────────────────────────────────
    async sessionCreate(name?: string): Promise<unknown> {
        return this.call('vox_session_create', name ? { name } : {});
    }
    async sessionList(): Promise<unknown[]> {
        const result = await this.call<unknown[]>('vox_session_list', {});
        return Array.isArray(result) ? result : [];
    }
    async sessionReset(): Promise<unknown> {
        return this.call('vox_session_reset', {});
    }

    // ── Behavior ──────────────────────────────────────────────────────────────
    async behaviorRecord(event: string, detail: string): Promise<void> {
        await this.call('vox_behavior_record', { event, detail });
    }

    // ── Git ───────────────────────────────────────────────────────────────────
    async gitStatus(): Promise<unknown> {
        return this.call('vox_git_status', {});
    }
    async gitLog(limit = 10): Promise<unknown[]> {
        const result = await this.call<unknown[]>('vox_git_log', { limit });
        return Array.isArray(result) ? result : [];
    }

    dispose(): void {
        clearTimeout(this._reconnectTimer);
        this.client.close().catch(() => void 0);
    }
}
