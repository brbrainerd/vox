import * as vscode from 'vscode';
import { VoxMcpClient } from '../core/VoxMcpClient';
import type { AgentEvent, AgentState, AgentRole, AgentStatus } from '../types';

export class AgentController {
    private _agents: Map<string, AgentState> = new Map();
    private _pollTimer?: NodeJS.Timeout;
    private _seenEventIds = new Set<string>();
    private _onUpdate: (agents: AgentState[]) => void;

    constructor(
        private readonly _mcp: VoxMcpClient,
        onUpdate: (agents: AgentState[]) => void,
    ) {
        this._onUpdate = onUpdate;
    }

    start(): void {
        this._pollTimer = setInterval(() => this._poll(), 3000);
    }

    stop(): void {
        clearInterval(this._pollTimer);
    }

    private async _poll(): Promise<void> {
        if (!this._mcp.connected) return;
        const events = await this._mcp.pollEvents(30);
        let changed = false;

        for (const ev of events.reverse()) {
            const id = ev.id ?? `${ev.agent_id}-${ev.timestamp}-${ev.event_type ?? ev.type}`;
            if (this._seenEventIds.has(id)) continue;
            this._seenEventIds.add(id);
            if (this._seenEventIds.size > 500) {
                const [first] = this._seenEventIds;
                this._seenEventIds.delete(first);
            }

            this._applyEvent(ev);
            changed = true;
        }

        if (changed) {
            this._onUpdate(this.getAgents());
        }
    }

    private _applyEvent(ev: AgentEvent): void {
        const agentId = ev.agent_id || 'system';
        const existing = this._agents.get(agentId);

        const status = this._inferStatus(ev.event_type ?? ev.type ?? '');
        const role = existing?.role ?? this._inferRole(agentId);

        this._agents.set(agentId, {
            agent_id: agentId,
            role,
            status,
            current_task: this._extractTask(ev),
            xp: existing?.xp ?? 0,
            mood: existing?.mood,
            last_event: ev,
            last_seen: Date.now(),
        });

        // Show desktop notification for completions
        if (ev.event_type === 'TaskCompleted' || ev.event_type === 'Completed') {
            vscode.window.showInformationMessage(
                `✓ Vox Agent [${agentId}] completed task`,
                'View Details'
            ).then(sel => {
                if (sel === 'View Details') {
                    vscode.commands.executeCommand('vox.focusSidebar');
                }
            });
        }

        // Show error notification
        if (ev.event_type === 'TaskFailed' || ev.event_type === 'Error') {
            vscode.window.showWarningMessage(
                `⚠ Vox Agent [${agentId}] encountered an error`,
                'Open Debug Mode'
            ).then(sel => {
                if (sel === 'Open Debug Mode') {
                    vscode.commands.executeCommand('vox.debugMode');
                }
            });
        }
    }

    private _inferStatus(eventType: string): AgentStatus {
        const lower = eventType.toLowerCase();
        if (lower.includes('complete') || lower.includes('done') || lower.includes('finish')) return 'done';
        if (lower.includes('fail') || lower.includes('error')) return 'error';
        if (lower.includes('start') || lower.includes('working') || lower.includes('token') || lower.includes('running')) return 'working';
        if (lower.includes('wait') || lower.includes('pending')) return 'waiting';
        return 'idle';
    }

    private _inferRole(agentId: string): AgentRole {
        const lower = agentId.toLowerCase();
        if (lower.includes('build')) return 'build';
        if (lower.includes('plan')) return 'plan';
        if (lower.includes('debug')) return 'debug';
        if (lower.includes('research')) return 'research';
        if (lower.includes('review')) return 'review';
        return 'unknown';
    }

    private _extractTask(ev: AgentEvent): string | undefined {
        try {
            const payload = ev.payload ? JSON.parse(ev.payload) : {};
            return payload.task_description || payload.description || payload.task || ev.message;
        } catch {
            return ev.message;
        }
    }

    getAgents(): AgentState[] {
        // Prune agents not seen in 5 minutes
        const cutoff = Date.now() - 5 * 60_000;
        for (const [id, agent] of this._agents) {
            if (agent.last_seen < cutoff && agent.status !== 'working') {
                this._agents.delete(id);
            }
        }
        return Array.from(this._agents.values());
    }

    async spawn(role: AgentRole, task: string, mode?: 'plan' | 'debug'): Promise<void> {
        if (mode === 'plan') {
            // Plan mode: submit with mode=plan flag, show the plan before executing
            const result = await this._mcp.submitTask(`[${role}] ${task}`, [], 'plan');
            const plan = (result as { plan?: string })?.plan;
            if (plan) {
                const choice = await vscode.window.showInformationMessage(
                    `Vox Plan:\n${plan.substring(0, 300)}...`,
                    { modal: true },
                    'Execute Plan',
                    'Cancel',
                );
                if (choice !== 'Execute Plan') return;
                await this._mcp.submitTask(`[${role}] ${task}`, [], 'execute');
            }
        } else {
            await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: `Spawning Vox ${role} agent...`,
                    cancellable: true,
                },
                async (_, token) => {
                    const taskPromise = this._mcp.submitTask(`[${role}] ${task}`, []);
                    token.onCancellationRequested(() => {
                        vscode.window.showInformationMessage('Agent spawn cancelled.');
                    });
                    await taskPromise;
                }
            );
        }
    }

    async kill(agentId: string): Promise<void> {
        const agent = this._agents.get(agentId);
        if (!agent) return;
        const confirm = await vscode.window.showWarningMessage(
            `Kill agent [${agentId}]? Its current task will be cancelled.`,
            { modal: true },
            'Kill Agent',
        );
        if (confirm !== 'Kill Agent') return;
        await this._mcp.cancelTask(agentId);
        this._agents.delete(agentId);
        this._onUpdate(this.getAgents());
    }

    async rebalance(): Promise<void> {
        await this._mcp.rebalance();
        vscode.window.showInformationMessage('Vox task queue rebalanced.');
    }

    async enableDebugMode(agentId?: string): Promise<void> {
        // Run workspace check and tests, feed results back as a debug task
        const [wsResult, testResult] = await Promise.all([
            this._mcp.checkWorkspace(),
            this._mcp.runTests(),
        ]);
        const diagnostics = `Workspace: ${JSON.stringify(wsResult)}\nTests: ${JSON.stringify(testResult)}`;
        await this._mcp.submitTask(
            `[debug] Analyze and fix these errors:\n${diagnostics}`,
            [],
            'debug'
        );
        vscode.window.showInformationMessage('Debug Mode: Vox is analyzing errors...');
    }
}
