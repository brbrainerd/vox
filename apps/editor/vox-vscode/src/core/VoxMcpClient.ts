import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import type {
    AgentEvent,
    ChatMessage,
    ChatSessionMeta,
    Snapshot,
    OplogEntry,
    BudgetStatus,
    GamifyState,
    SkillInfo,
    VoxConfigResponse,
    SubmitTaskResponse,
    PlanGoalResult,
    AttentionStatusPayload,
    AttentionHistoryParams,
    AttentionHistoryPayload,
} from '../types';
import { CapabilityRegistry, type ListedMcpTool } from './CapabilityRegistry';
import { MCP_EXTENSION_EXPECTED_TOOLS } from './mcpToolRegistry.generated';
import { ConfigManager } from './ConfigManager';
import { parseMcpToolResult, unwrapVoxToolEnvelope } from './mcpToolResult';

export class VoxMcpClient {
    private client: Client;
    private transport: StdioClientTransport;
    private _connected = false;
    private _reconnectDelay = 1000;
    private _reconnectTimer?: NodeJS.Timeout;
    public outputChannel: vscode.OutputChannel;
    private readonly _serverPath: string;
    readonly capabilities = new CapabilityRegistry();

    constructor(outputChannel: vscode.OutputChannel, serverPath = 'vox') {
        this.outputChannel = outputChannel;
        this._serverPath = serverPath;
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

    private _newClient(): Client {
        return new Client(
            { name: 'vox-vscode-client', version: '0.2.0' },
            { capabilities: {} }
        );
    }

    async connect(): Promise<void> {
        try {
            this.outputChannel.appendLine(`[Vox MCP] Connecting via StdioClientTransport...`);
            
            let effectivePath = this._serverPath;
            if (effectivePath === 'vox' || !effectivePath) {
                // If it's pure 'vox' and not in PATH natively, or testing locally in VS Code
                // We'll peek to see if the adjacent target/debug exists.
                if (vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders.length > 0) {
                    const wsRoot = vscode.workspace.workspaceFolders[0].uri.fsPath;
                    const possibleParent = path.dirname(wsRoot); 
                    // Usually vox-vscode is inside vox dir
                    const localDebug = path.join(possibleParent, 'target', 'debug', process.platform === 'win32' ? 'vox.exe' : 'vox');
                    if (fs.existsSync(localDebug)) {
                        effectivePath = localDebug;
                        this.outputChannel.appendLine(`[Vox MCP] Overriding 'vox' default with local workspace binary: ${effectivePath}`);
                    }
                }
            }

            this.outputChannel.appendLine(`[Vox MCP] Binary Path finalized as: ${effectivePath}`);
            if (effectivePath !== 'vox' && !fs.existsSync(effectivePath)) {
                throw new Error(`Configured MCP server path does not exist on disk: ${effectivePath}`);
            }
            
            this.transport = new StdioClientTransport({ command: effectivePath, args: ['mcp'] });
            
            await this.client.connect(this.transport);
            this._connected = true;
            this._reconnectDelay = 1000;
            this.outputChannel.appendLine(`[Vox MCP] Transport connected! Fetching listTools...`);
            const tools = await this.client.listTools();
            const listed: ListedMcpTool[] = tools.tools.map((t) => ({
                name: t.name,
                inputSchema: t.inputSchema as object | undefined,
            }));
            this.capabilities.refreshFromList(listed);
            this.outputChannel.appendLine(
                `[Vox MCP] Connected. ${tools.tools.length} tools (fp=${this.capabilities.schemaFingerprint}).`,
            );
            if (ConfigManager.mcpWarnOnMissingTools) {
                const absent = this.capabilities.missingFromList(MCP_EXTENSION_EXPECTED_TOOLS);
                if (absent.length > 0) {
                    this.outputChannel.appendLine(
                        `[Vox MCP] Expected tools missing from list_tools (UI may degrade): ${absent.join(', ')}`,
                    );
                }
            }
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : String(e);
            this.outputChannel.appendLine(`[Vox MCP] Failed to connect: ${msg}`);
            this._connected = false;
            this.capabilities.lastError = msg;
            this._scheduleReconnect();
        }
    }

    private _scheduleReconnect(): void {
        clearTimeout(this._reconnectTimer);
        this._reconnectTimer = setTimeout(async () => {
            this.outputChannel.appendLine(`[Vox MCP] Reconnecting (delay: ${this._reconnectDelay}ms)...`);
            this.transport = new StdioClientTransport({ command: this._serverPath, args: ['mcp'] });
            this.client = this._newClient();
            this.client.fallbackNotificationHandler = async (notification) => {
                this.outputChannel.appendLine(`[MCP Notification] ${JSON.stringify(notification)}`);
            };
            await this.connect();
            this._reconnectDelay = Math.min(this._reconnectDelay * 2, 30000);
        }, this._reconnectDelay);
    }

    /** Public accessor so thin-client wrappers can call arbitrary MCP tools without boilerplate. */
    async call<T>(name: string, args: Record<string, unknown>): Promise<T | null> {
        if (!this._connected) return null;
        try {
            const debug =
                vscode.workspace.getConfiguration('vox').get<boolean>('mcp.debugPayloads', false);
            if (debug) {
                this.outputChannel.appendLine(`[Vox MCP] call ${name} args=${JSON.stringify(args).slice(0, 2000)}`);
            }
            const result = await this.client.callTool({ name, arguments: args });
            const parsed = parseMcpToolResult(result);
            const unwrapped = unwrapVoxToolEnvelope(parsed, this.outputChannel, name);
            if (debug && unwrapped !== undefined) {
                this.outputChannel.appendLine(
                    `[Vox MCP] ${name} result=${JSON.stringify(unwrapped).slice(0, 2000)}`,
                );
            }
            return unwrapped as T | null;
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

    /** Returns false if the server did not advertise this tool name (skills may add more at runtime). */
    isToolAvailable(name: string): boolean {
        return this.capabilities.has(name);
    }

    // ── Task & Orchestration ──────────────────────────────────────────────────
    async submitTask(
        description: string,
        files: string[] = [],
        mode?: string,
        traceId?: string,
        accessMode: 'read' | 'write' = 'write'
    ): Promise<SubmitTaskResponse | null> {
        const fileSpecs =
            files.length > 0
                ? files.map((path) => ({ path, access: accessMode }))
                : [{ path: '.', access: accessMode }];

        const tool_hints: string[] = [];
        const research_hints: string[] = [];
        const toolRegex = /\[\[tool:([^\]]+)\]\]/g;
        const researchRegex = /\[\[research:([^\]]+)\]\]/g;

        let match;
        while ((match = toolRegex.exec(description)) !== null) {
            tool_hints.push(match[1]);
        }
        while ((match = researchRegex.exec(description)) !== null) {
            research_hints.push(match[1]);
        }

        const payload: Record<string, unknown> = {
            description,
            files: fileSpecs,
            tool_hints,
            research_hints,
        };
        if (mode) payload.planning_mode = mode;
        const tid =
            traceId ??
            (globalThis.crypto && 'randomUUID' in globalThis.crypto
                ? globalThis.crypto.randomUUID()
                : undefined);
        if (tid) payload.trace_id = tid;
        const result = await this.call<SubmitTaskResponse>('vox_submit_task', payload);

        if (result?.shadow_plan_adequacy) {
            const sh = result.shadow_plan_adequacy;
            this.outputChannel.appendLine(
                `[Vox MCP] shadow_plan_adequacy score=${sh.score.toFixed(3)} too_thin=${sh.is_too_thin} ` +
                    `critical=${sh.critical_count} risk=${sh.aggregate_unresolved_risk.toFixed(3)} ` +
                    `codes=[${sh.reason_codes.join(', ')}]`,
            );
            if (
                sh.is_too_thin &&
                vscode.workspace.getConfiguration('vox').get<boolean>('mcp.shadowPlanAdequacyToast', true)
            ) {
                const preview =
                    sh.reason_codes.slice(0, 3).join('; ') || 'plan may be too thin (shadow heuristic)';
                void vscode.window
                    .showWarningMessage(
                            `Vox: task submitted, but plan-adequacy shadow flagged a thin plan: ${preview}`,
                            'Open Vox Log',
                    )
                    .then((choice) => {
                        if (choice === 'Open Vox Log') this.outputChannel.show(true);
                    });
            }
        }
        return result;
    }
    async taskStatus(taskId: string): Promise<unknown> {
        return this.call('vox_task_status', { task_id: Number(taskId) });
    }
    async completeTask(taskId: string): Promise<unknown> {
        return this.call('vox_complete_task', { task_id: Number(taskId) });
    }
    async cancelTask(taskId: string): Promise<unknown> {
        return this.call('vox_cancel_task', { task_id: Number(taskId) });
    }

    /** Mark a task as doubted (HITL); `task_id` must parse to a non-negative integer. */
    async doubtTask(taskId: string, reason?: string): Promise<unknown> {
        const id = Number(taskId);
        if (!Number.isFinite(id) || id < 0 || !Number.isInteger(id)) {
            this.outputChannel.appendLine(`[Vox MCP] doubtTask: invalid task_id "${taskId}"`);
            return null;
        }
        const args: Record<string, unknown> = { task_id: id };
        if (reason !== undefined && reason.length > 0) {
            args.reason = reason;
        }
        return this.call('vox_doubt_task', args);
    }

    async orchestratorStatus(): Promise<GamifyState | null> {
        return this.call<GamifyState>('vox_orchestrator_status', {});
    }

    /** Ludus KPI + unread notifications + recent policy (requires Codex + tools on server). */
    async ludusProgressSnapshot(params?: {
        notification_limit?: number;
        policy_limit?: number;
        policy_days?: number;
    }): Promise<Record<string, unknown> | null> {
        return this.call<Record<string, unknown>>('vox_gamify_progress_snapshot', {
            notification_limit: params?.notification_limit ?? 12,
            policy_limit: params?.policy_limit ?? 24,
            policy_days: params?.policy_days ?? 7,
        });
    }

    async ludusNotificationAck(notificationId: string): Promise<unknown> {
        return this.call('vox_gamify_notification_ack', { notification_id: notificationId });
    }

    async ludusNotificationsAckAll(): Promise<unknown> {
        return this.call('vox_gamify_notifications_ack_all', {});
    }
    async rebalance(): Promise<unknown> {
        return this.call('vox_rebalance', {});
    }
    async emergencyStop(reason?: string): Promise<unknown> {
        return this.call('vox_emergency_stop', { reason });
    }

    async spawnAgent(name: string, dynamic = false): Promise<unknown> {
        return this.call('vox_spawn_agent', { name, dynamic });
    }
    async retireAgent(agentId: number): Promise<unknown> {
        return this.call('vox_retire_agent', { agent_id: agentId });
    }
    async pauseAgent(agentId: number): Promise<unknown> {
        return this.call('vox_pause_agent', { agent_id: agentId });
    }
    async resumeAgent(agentId: number): Promise<unknown> {
        return this.call('vox_resume_agent', { agent_id: agentId });
    }
    async drainAgent(agentId: number): Promise<unknown> {
        return this.call('vox_drain_agent', { agent_id: agentId });
    }
    async pollEvents(limit = 20): Promise<AgentEvent[]> {
        const result = await this.call<AgentEvent[]>('vox_poll_events', { limit });
        return Array.isArray(result) ? result : [];
    }

    // ── Cost / models (canonical tool names + server-side aliases) ─────────────
    async budgetHistory(buckets = 20): Promise<unknown[]> {
        const result = await this.call<unknown[]>('vox_cost_history', { limit_per_agent: buckets });
        return Array.isArray(result) ? result : [];
    }
    async modelList(): Promise<unknown[]> {
        const result = await this.call<unknown[]>('vox_list_models', {});
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
        if (!crate) {
            return this.call('vox_check_workspace', {});
        }
        return this.call('vox_run_tests', { crate_name: crate });
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
    async setAgentBudget(agentId: number, maxTokens?: number, maxCostUsd?: number): Promise<unknown> {
        return this.call('vox_set_agent_budget', {
            agent_id: agentId,
            max_tokens: maxTokens,
            max_cost_usd: maxCostUsd,
        });
    }

    // ── Attention Budget ──────────────────────────────────────────────────────
    async attentionStatus(): Promise<AttentionStatusPayload | null> {
        return this.call<AttentionStatusPayload>('vox_attention_summary', {});
    }

    async attentionReset(confirm: string, newMaxMs?: number): Promise<void> {
        await this.call('vox_attention_reset', { confirm, new_max_ms: newMaxMs });
    }

    async attentionHistory(params: AttentionHistoryParams): Promise<AttentionHistoryPayload | null> {
        return this.call<AttentionHistoryPayload>('vox_attention_history', {
            since_hours: params.since_hours,
            channel: params.channel,
            agent_id: params.agent_id,
            limit: params.limit,
        });
    }

    async trustOverride(agentId: number, tier: string, reason: string): Promise<void> {
        await this.call('vox_trust_override', { agent_id: agentId, tier, reason });
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
    async a2aTasks(): Promise<unknown[]> {
        const result = await this.call<unknown[]>('vox_a2a_tasks', {});
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

    async chatHistory(sessionId?: string): Promise<ChatMessage[] | null> {
        const payload =
            sessionId != null && sessionId !== ''
                ? { session_id: sessionId }
                : {};
        return this.call<ChatMessage[]>('vox_chat_history', payload);
    }

    async chatMessage(
        payload: {
            prompt: string;
            contextFiles?: string[];
            openFiles?: string[];
            activeFile?: string;
            activeLine?: number;
            selectedText?: string;
            diagnostics?: unknown[];
            sessionId?: string;
            cognitiveProfile?: 'fast' | 'reasoning' | 'creative';
        },
    ): Promise<{
        message: ChatMessage;
        history: ChatMessage[];
        model_used?: string;
        tokens?: number;
        session_id?: string;
        socrates?: ChatSessionMeta['socrates'];
        retrieval?: ChatSessionMeta['retrieval'];
    } | null> {
        const trace_id =
            globalThis.crypto && 'randomUUID' in globalThis.crypto
                ? globalThis.crypto.randomUUID()
                : `vscode-chat-${Date.now()}`;
        return this.call('vox_chat_message', {
            prompt: payload.prompt,
            context_files: payload.contextFiles ?? [],
            open_files: payload.openFiles ?? [],
            active_file: payload.activeFile,
            active_line: payload.activeLine,
            selected_text: payload.selectedText,
            diagnostics: payload.diagnostics ?? [],
            session_id: payload.sessionId,
            cognitive_profile: payload.cognitiveProfile,
            trace_id,
            correlation_id: trace_id,
            ...(payload.sessionId ? { thread_id: payload.sessionId } : {}),
            journey_id: trace_id,
        });
    }

    async configGet(): Promise<VoxConfigResponse | null> {
        return this.call<VoxConfigResponse>('vox_config_get', {});
    }

    async configSet(key: string, value: string): Promise<unknown> {
        return this.call('vox_config_set', { key, value });
    }

    async suggestModel<T = unknown>(taskCategory: string): Promise<T | null> {
        return this.call<T>('vox_suggest_model', { task_category: taskCategory });
    }

    async inlineEdit(payload: Record<string, unknown>): Promise<unknown> {
        return this.call('vox_inline_edit', payload);
    }

    async replanSession(payload: {
        session_id: string;
        delta_hint: string;
        write_to_disk: boolean;
    }): Promise<unknown> {
        return this.call('vox_replan', payload);
    }

    async planGoal(payload: {
        goal: string;
        write_to_disk: boolean;
        max_tasks: number;
        plan_depth?: 'minimal' | 'standard' | 'deep';
        questioning_hints_enabled?: boolean;
        session_id?: string;
    }): Promise<PlanGoalResult | null> {
        return this.call('vox_plan', payload);
    }

    async repoIndexStatus(): Promise<unknown> {
        return this.call('vox_repo_index_status', {});
    }

    async repoIndexRefresh(): Promise<unknown> {
        return this.call('vox_repo_index_refresh', {});
    }

    async repoCatalogList(): Promise<unknown> {
        return this.call('vox_repo_catalog_list', {});
    }

    async repoCatalogRefresh(): Promise<unknown> {
        return this.call('vox_repo_catalog_refresh', {});
    }

    async repoQueryText(payload: { query: string; limit?: number; repository_ids?: string[] }): Promise<unknown> {
        return this.call('vox_repo_query_text', payload);
    }

    async repoQueryFile(payload: { path: string; repository_ids?: string[] }): Promise<unknown> {
        return this.call('vox_repo_query_file', payload);
    }

    async capabilityModelManifest(): Promise<unknown> {
        return this.call('vox_capability_model_manifest', {});
    }

    async listContext(prefix: string): Promise<string[]> {
        const result = await this.call<string[]>('vox_list_context', { prefix });
        return Array.isArray(result) ? result : [];
    }

    async getContext(key: string): Promise<unknown> {
        return this.call('vox_get_context', { key });
    }

    async setContext(payload: {
        agent_id: number;
        key: string;
        value: string;
        ttl_seconds?: number;
    }): Promise<unknown> {
        return this.call('vox_set_context', payload);
    }

    async browserOpen(url: string): Promise<unknown> {
        return this.call('vox_browser_open', { url });
    }

    async browserGoto(page_id: string | number, url: string): Promise<unknown> {
        return this.call('vox_browser_goto', { page_id, url });
    }

    async browserText(payload: { page_id: string | number; target: string }): Promise<unknown> {
        return this.call('vox_browser_text', payload);
    }

    async browserExtract(payload: {
        page_id: string | number;
        instruction: string;
    }): Promise<unknown> {
        return this.call('vox_browser_extract', payload);
    }

    async browserScreenshot(payload: {
        page_id: string | number;
        path: string;
    }): Promise<unknown> {
        return this.call('vox_browser_screenshot', payload);
    }

    async projectInit(payload: {
        project_name: string;
        package_kind?: string;
        template?: string;
        target_subdir?: string;
    }): Promise<unknown> {
        return this.call('vox_project_init', payload);
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

    // ── Introspection ────────────────────────────────────────────────────────
    async languageSurface(): Promise<unknown> {
        return this.call('vox_language_surface', {});
    }
    async astInspect(path: string): Promise<unknown> {
        return this.call('vox_compiler::ast_inspect', { path });
    }
    async pipelineStatus(): Promise<unknown> {
        return this.call('vox_pipeline_status', {});
    }
    async decoratorRegistry(): Promise<unknown[]> {
        const result = await this.call<unknown[]>('vox_decorator_registry', {});
        return Array.isArray(result) ? result : [];
    }
    async builtinRegistry(): Promise<unknown[]> {
        const result = await this.call<unknown[]>('vox_builtin_registry', {});
        return Array.isArray(result) ? result : [];
    }

    // ── Oratio / speech pipeline ────────────────────────────────────────────
    async oratioTranscribe(
        path: string,
        opts?: {
            language_hint?: string;
            profile?: 'conservative' | 'balanced' | 'aggressive';
            debug_parser_payload?: boolean;
        },
    ): Promise<unknown> {
        const args: Record<string, unknown> = { path };
        if (opts?.language_hint !== undefined) args.language_hint = opts.language_hint;
        if (opts?.profile !== undefined) args.profile = opts.profile;
        if (opts?.debug_parser_payload !== undefined) {
            args.debug_parser_payload = opts.debug_parser_payload;
        }
        return this.call('vox_oratio_transcribe', args);
    }

    async oratioStatus(): Promise<{
        summary?: string;
        streaming?: { stream_ws_url?: string };
    } | null> {
        return this.call('vox_oratio_status', {});
    }

    async speechToCode(payload: {
        path?: string;
        prompt?: string;
        language_hint?: string;
        profile?: 'conservative' | 'balanced' | 'aggressive';
        debug_parser_payload?: boolean;
        route_mode?: 'none' | 'tool' | 'chat' | 'orchestrator';
        include_route?: boolean;
        validate?: boolean;
        max_retries?: number;
        session_id?: string;
        output_surface_mode?: string;
        emit_trace_path?: string;
    }): Promise<unknown> {
        const args: Record<string, unknown> = {};
        for (const [k, v] of Object.entries(payload)) {
            if (v !== undefined) args[k] = v;
        }
        return this.call('vox_speech_to_code', args);
    }

    dispose(): void {
        clearTimeout(this._reconnectTimer);
        this.client.close().catch(() => void 0);
    }
}
