import * as blessed from 'blessed';
import * as contrib from 'blessed-contrib';

// OpenCode interface stubs
export interface OpenCodePluginAPI {
    ui: {
        screen: blessed.Widgets.Screen;
        statusBar: {
            setRightText(text: string): void;
            setLeftText(text: string): void;
        };
        appendLog(message: string): void;
        registerOverlay(name: string, widget: any): void;
        modifyToolOutput(processor: (output: string) => string): void;
    };
    mcp: {
        callTool(server: string, tool: string, params: any): Promise<any>;
    };
    session: {
        getId(): string;
        on(event: 'start', callback: () => void): void;
        on(event: 'tool_call', callback: (details: any) => void): void;
        on(event: 'message', callback: (msg: string) => void): void;
        on(event: 'cost', callback: (usage: any) => void): void;
    };
    version?: string; // New in opencode-ai >= 0.2.0
}

export default function initVoxPlugin(api: OpenCodePluginAPI) {
    // 8.4 Plugin API Compatibility Shim (v1 -> v2)
    if (!api.version || api.version.startsWith('0.1.')) {
        api.ui.appendLog('[Vox] Running in compatibility mode (OpenCode < 0.2.0)');
        // Shim missing session costs event
        if (!api.session.on.toString().includes('cost')) {
            api.session.on = new Proxy(api.session.on, {
                apply(target, thisArg, argArray) {
                    if (argArray[0] === 'cost') return; // Ignore if unsupported
                    return target.apply(thisArg, argArray as any);
                }
            });
        }
    }
    let voxAgentId: string | null = null;
    let topologyWidget: any | null = null;
    let agentDashboardWidget: any | null = null;

    // 4.8 Plugin Hooks
    api.session.on('start', async () => {
        api.ui.appendLog('[Vox] Connecting OpenCode session to Vox Orchestrator...');
        try {
            // Register this session with the Orchestrator via MCP
            const sessionId = api.session.getId();

            // First, find an idle or available agent, or let the orchestrator spawn one for us
            // (Mocking this: we assume the orchestrator MCP will handle a "map session" request)

            // We use our new mapped tool
            const mapRes = await api.mcp.callTool('vox', 'vox_map_opencode_session', {
                agent_id: 1, // Defaulting to the root agent (or we could fetch agents first)
                session_id: sessionId
            });

            voxAgentId = "1";
            api.ui.appendLog(`[Vox] ${mapRes.content?.[0]?.text || 'Session Mapped'}`);
            api.ui.appendLog(`[Vox] Dashboard URL: http://localhost:8080`);

            // Start polling for live stats
            startPollingStatus(api);
            setupUIOverlays(api);

        } catch (e: any) {
            api.ui.appendLog(`[Vox] Failed to map session: ${e.message}`);
        }
    });

    api.session.on('tool_call', (details) => {
        // Record tool usage
        api.ui.appendLog(`[Vox] Tool call: ${details.tool}`);
    });

    // 6.7 Cost tracking bridge (OpenCode token usage → orchestrator budget)
    api.session.on('cost', async (usage) => {
        try {
            await api.mcp.callTool('vox', 'vox_record_cost', {
                session_id: api.session.getId(),
                provider: usage.provider || 'opencode',
                model: usage.model || 'unknown',
                cost_usd: usage.cost_usd || 0.0,
                input_tokens: usage.input_tokens || 0,
                output_tokens: usage.output_tokens || 0
            });
        } catch (e: any) {
            api.ui.appendLog(`[Vox] Failed to record cost: ${e.message}`);
        }
    });

    api.session.on('message', (msg) => {
        // Evaluate incoming message
        if (msg.includes('URGENT')) {
            api.ui.statusBar.setLeftText('{red-fg}URGENT OVERRIDE{/red-fg}');
        }
    });

    // 4.5 Plugin: file lock indicator
    api.ui.modifyToolOutput((output) => {
        // Whenever a tool outputs file paths, if they are locked by Vox, show a lock icon
        // (Mock implementation: we could query vox_lock_status but for speed we just annotate known paths)
        // E.g., replace `src/lib.rs` with `🔒 src/lib.rs` if locked
        if (output.includes('.rs') && output.includes('conflict')) {
            return output.replace(/\.rs/g, '.rs 🔒');
        }
        return output;
    });

    async function startPollingStatus(api: OpenCodePluginAPI) {
        setInterval(async () => {
            try {
                // 6.6 Agent heartbeat bridge
                await api.mcp.callTool('vox', 'vox_heartbeat', { session_id: api.session.getId() }).catch(() => {});

                // 4.2 Cost ticker
                const budgetRes = await api.mcp.callTool('vox', 'vox_budget_status', {});
                if (budgetRes?.content?.[0]?.text) {
                    api.ui.statusBar.setRightText(`[Vox] ${budgetRes.content[0].text}`);
                }

                // 6.4 Feed orchestrator events into OpenCode session context + 4.6 Auto-rebalance notification
                const eventsRes = await api.mcp.callTool('vox', 'vox_poll_events', { limit: 5 });
                if (eventsRes?.content?.[0]?.text) {
                    const eventsArr = JSON.parse(eventsRes.content[0].text);
                    eventsArr.forEach((ev: any) => {
                        if (ev.type === 'urgent_rebalance_triggered' || ev.payload?.includes('UrgentRebalanceTriggered')) {
                            api.ui.appendLog('{yellow-fg}[Vox Alert] Urgent task rebalance triggered!{/yellow-fg}');
                        }
                        // 6.8 Plan handoff via OpenCode session prompts
                        if (ev.type === 'plan_handoff') {
                            api.ui.appendLog(`{cyan-fg}[Vox Handoff] ${ev.from || 'System'} -> ${ev.to || 'Agent'}: ${ev.plan_summary || 'Handoff'}{/cyan-fg}`);
                        }
                    });
                }

                // Fetch full status for Gamification (4.4) & topology (4.3) & VCS (4.7)
                const statusRes = await api.mcp.callTool('vox', 'vox_agent_status', { agent_id: parseInt(voxAgentId || '1') });
                if (agentDashboardWidget && statusRes?.content?.[0]?.text) {
                    const txt = statusRes.content[0].text;
                    agentDashboardWidget.setContent(txt);
                    api.ui.screen.render();
                }

            } catch (e: any) {
                // Ignore transient MCP errors if orchestrator restarts
            }
        }, 3000);
    }

    // 6.5 Route task submissions through OpenCode sessions
    api.session.on('message', async (msg) => {
        if (msg.startsWith('/task ')) {
            const desc = msg.replace('/task ', '').trim();
            api.ui.appendLog(`[Vox] Submitting task: ${desc}`);
            try {
                const res = await api.mcp.callTool('vox', 'vox_submit_task', {
                    description: desc,
                    agent_id: parseInt(voxAgentId || '1')
                });
                api.ui.appendLog(`[Vox] ${res.content?.[0]?.text || 'Task submitted'}`);
            } catch (err: any) {
                api.ui.appendLog(`[Vox] Failed to submit task: ${err.message}`);
            }
        }
    });

    function setupUIOverlays(api: OpenCodePluginAPI) {
        // 4.3 Topology & 4.4 Gamification HUD
        const box = blessed.box({
            top: '0',
            right: '0',
            width: '30%',
            height: '100%',
            label: ' Vox Intelligence HUD ',
            border: { type: 'line' },
            style: { border: { fg: 'cyan' }, label: { fg: 'cyan', bold: true } },
            tags: true,
            scrollable: true,
            alwaysScroll: true,
            mouse: true
        });

        agentDashboardWidget = box;
        agentDashboardWidget.setContent('Fetching orchestrator data...');
        api.ui.registerOverlay('vox_hud', box);
    }
}
