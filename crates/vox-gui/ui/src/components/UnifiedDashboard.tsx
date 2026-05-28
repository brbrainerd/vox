import React from 'react';
import { Terminal, Activity, Trophy, Bell, Cpu, Layers, MessageSquare, AlertCircle, CheckCircle2, Sparkles } from 'lucide-react';
import { voxTransport } from '../transport';
// import { Panel } from './ui/Panel';
import { StateChip } from './ui/StateChip';

import { AttentionPanel } from './AttentionPanel';

function opRowTone(status: string): 'success' | 'warning' | 'danger' | 'neutral' | 'info' {
    const normalized = status.toLowerCase();
    if (normalized.includes('run')) return 'info';
    if (normalized.includes('success') || normalized.includes('complete') || normalized.includes('validated')) return 'success';
    if (normalized.includes('fail') || normalized.includes('error') || normalized.includes('overruled')) return 'danger';
    if (normalized.includes('queue') || normalized.includes('block') || normalized.includes('suspect') || normalized.includes('doubt')) return 'warning';
    return 'neutral';
}

export const UnifiedDashboard = ({
    ops = [],
    stats = {},
    pipeline = null,
    budgetHistory = [],
    ludusSnapshot = null,
    _meshTopology = null,
    attentionStatus = null,
    attentionAlert = null,
}: {
    ops: any[];
    stats: any;
    pipeline: any;
    budgetHistory: any[];
    ludusSnapshot: Record<string, unknown> | null;
    _meshTopology: any;
    attentionStatus: any;
    attentionAlert: any;
}) => {
    const isIdle = (stats.activeAgents === '0' || !stats.activeAgents) && (stats.queueDepth === '0' || !stats.queueDepth) && ops.length === 0;

    return (
        <div className="p-4 pb-20 grid grid-cols-12 gap-4 overflow-y-auto flex-1 min-h-0 text-foreground custom-scrollbar relative z-10 w-full h-full bg-background border-t border-border">
            {/* Header Area */}
            <div className="col-span-12 flex justify-between items-center mb-2 pb-2 border-b border-border border-opacity-50">
                <h2 className="text-2xl font-rajdhani text-brass tracking-wider">IMPERIUM</h2>
                <div className="flex gap-4 text-[10px] font-mono text-cyan uppercase tracking-widest px-3 py-1 bg-cyan bg-opacity-10 rounded border border-cyan border-opacity-30">
                    <span className="flex items-center gap-1"><Cpu size={12} className="text-primary"/> Agents: {stats.activeAgents ?? "0"}</span>
                    <span className="flex items-center gap-1 text-steel">|</span>
                    <span className="flex items-center gap-1"><Layers size={12} className="text-primary"/> Queue: {stats.queueDepth ?? "0"}</span>
                    <span className="flex items-center gap-1 text-steel">|</span>
                    <span className="flex items-center gap-1 text-destructive"><AlertCircle size={12}/> Doubted: {stats.totalDoubted ?? "0"}</span>
                    {stats.budget && (
                        <>
                            <span className="flex items-center gap-1 text-steel">|</span>
                            <span className="flex items-center gap-1"><Activity size={12} className="text-secondary-foreground" /> Budget: {stats.budget}</span>
                        </>
                    )}
                </div>
            </div>

            {/* Ludus KPI & Budget Header Widget */}
            {(ludusSnapshot?.kpi || budgetHistory.length > 0) && (
                <div className="col-span-12 grid grid-cols-12 gap-4">
                    {ludusSnapshot?.kpi && (
                        <div className="col-span-6 p-4 flex gap-4 items-center bg-machine border border-border rounded-lg shadow-[inset_0_2px_10px_rgba(0,0,0,0.5)]">
                            <Trophy size={20} className="text-primary drop-shadow-[0_0_8px_var(--vox-amber-glow)]" />
                            <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 flex-1 text-xs font-mono text-cyan">
                                <div><div className="text-steel text-[9px] tracking-widest uppercase mb-1">Events</div>{String((ludusSnapshot.kpi as any)?.events_recorded ?? '0')}</div>
                                <div><div className="text-steel text-[9px] tracking-widest uppercase mb-1">XP</div>{String((ludusSnapshot.kpi as any)?.total_xp_from_policy ?? (ludusSnapshot.kpi as any)?.total_xp ?? '0')}</div>
                                <div><div className="text-steel text-[9px] tracking-widest uppercase mb-1">Crystals</div>{String((ludusSnapshot.kpi as any)?.total_crystals_from_policy ?? (ludusSnapshot.kpi as any)?.total_crystals ?? '0')}</div>
                                <div><div className="text-steel text-[9px] tracking-widest uppercase mb-1">Streak</div>{String((ludusSnapshot.kpi as any)?.streak_days ?? '0')}</div>
                            </div>
                        </div>
                    )}
                    
                    {budgetHistory.length > 0 && (
                        <div className="col-span-6 p-4 flex flex-col gap-3 bg-machine border border-border rounded-lg shadow-[inset_0_2px_10px_rgba(0,0,0,0.5)]">
                            <div className="flex gap-4 items-center">
                                <Activity size={20} className="text-cyan drop-shadow-[0_0_8px_var(--vox-cyan-glow)] shrink-0" />
                                <div className="flex-1 text-xs overflow-hidden text-ellipsis whitespace-nowrap font-mono text-cyan">
                                    <div className="text-steel text-[9px] tracking-widest uppercase mb-1">Recent Budget Spend</div>
                                    ${parseFloat(budgetHistory[budgetHistory.length - 1]?.total_cost_usd || 0).toFixed(4)}
                                </div>
                            </div>
                            <div className="border-t border-border border-opacity-50 pt-3 mt-1 flex items-center gap-2">
                                <span className="text-[10px] text-steel font-mono uppercase tracking-widest label">Set Limit ($)</span>
                                <input 
                                    type="number" 
                                    id="budget-cap-input"
                                    placeholder="5.00" 
                                    className="bg-void border border-border rounded px-2 py-1 text-xs text-foreground font-mono w-24 focus:border-cyan focus:outline-none transition-colors"
                                />
                                <button
                                    onClick={() => {
                                        const el = document.getElementById('budget-cap-input') as HTMLInputElement;
                                        if (el?.value) {
                                            voxTransport.callTool('vox_set_agent_budget', { 
                                                agent_id: 0, 
                                                max_cost_usd: parseFloat(el.value) 
                                            });
                                            el.value = '';
                                        }
                                    }}
                                    className="text-[9px] font-bold tracking-widest uppercase bg-cyan bg-opacity-10 text-cyan border border-cyan border-opacity-30 rounded px-3 py-1.5 hover:bg-opacity-20 transition-colors shrink-0"
                                >
                                    APPLY
                                </button>
                            </div>
                        </div>
                    )}
                </div>
            )}

            {/* Notifications (Ludus) */}
            {(ludusSnapshot?.notifications as any[])?.length > 0 && (
                <div className="col-span-12">
                    <div className="p-4 rounded-lg border border-destructive bg-destructive bg-opacity-10 shadow-[0_0_15px_rgba(239,68,68,0.2)]">
                        <div className="flex justify-between items-center mb-3">
                            <div className="flex items-center gap-2 font-bold text-[10px] uppercase tracking-widest text-destructive">
                                <Bell size={14} className="animate-pulse" /> Unread Notifications
                            </div>
                            <button
                                type="button"
                                className="text-[9px] font-bold uppercase tracking-widest px-3 py-1 rounded border border-destructive bg-void text-destructive hover:bg-destructive hover:text-white transition-colors"
                                onClick={() => voxTransport.callTool('vox_gamify_notifications_ack_all', {})}
                            >
                                ACK ALL
                            </button>
                        </div>
                        <div className="space-y-2">
                            {((ludusSnapshot?.notifications as any) || []).map((n: any, i: number) => (
                                <div key={n.id ?? i} className="flex gap-2 items-center text-xs p-2 rounded bg-void border border-border font-mono text-steel">
                                    <div className="flex-1 truncate"><strong className="text-brass uppercase">{n.title ?? n.heading}</strong> <span className="mx-2 opacity-50">|</span> {n.body ?? n.message}</div>
                                    <button
                                        type="button"
                                        className="text-[10px] px-3 py-1 bg-machine border border-border text-steel rounded hover:border-cyan hover:text-cyan transition-colors uppercase tracking-widest font-bold"
                                        onClick={() => voxTransport.callTool('vox_gamify_notification_ack', { notification_id: String(n.id ?? '') })}
                                    >
                                        ACK
                                    </button>
                                </div>
                            ))}
                        </div>
                    </div>
                </div>
            )}

            {/* Main Operations Stream - Conditional on Idle State */}
            {isIdle ? (
                <div className="col-span-12 flex flex-col items-center justify-center p-12 border border-border bg-machine bg-opacity-50 rounded-xl shadow-[inset_0_5px_15px_rgba(0,0,0,0.5)] min-h-[400px]">
                    <div className="w-16 h-16 rounded-full border border-copper text-primary flex items-center justify-center mb-6 shadow-[0_0_15px_var(--vox-amber-glow)] relative">
                        <div className="absolute inset-0 rounded-full border border-primary animate-ping opacity-20" />
                        <Sparkles size={24} />
                    </div>
                    <h3 className="font-rajdhani text-2xl text-brass uppercase tracking-widest mb-3">Orchestrator Idle</h3>
                    <p className="text-steel font-mono text-xs mb-8 text-center max-w-md">
                        Network resources are currently standing by. No active agents or queued tasks. Select files and execute a prompt in <span className="text-primary">Loquela</span> to begin, or create a new task below.
                    </p>
                    <button 
                        onClick={() => {
                            voxTransport.callTool('vox_submit_task', { description: 'New task requested via dashboard' });
                        }}
                        className="px-8 py-3 bg-primary text-black font-rajdhani font-bold text-lg tracking-widest rounded uppercase hover:bg-amber-400 border border-transparent shadow-[0_0_10px_var(--vox-amber-glow)] transition-all"
                    >
                        NEW TASK
                    </button>
                </div>
            ) : (
                <div className="col-span-8">
                    <div className="flex-1 p-4 flex flex-col min-h-[400px] border border-border bg-surface rounded-xl shadow-lg relative overflow-hidden">
                        <div className="absolute top-0 right-0 p-4 opacity-5 pointer-events-none">
                            <Terminal size={120} />
                        </div>
                        <div className="flex items-center justify-between mb-4 relative z-10 border-b border-border pb-3">
                            <div className="flex items-center gap-2">
                                <Terminal size={16} className="text-secondary-foreground" />
                                <h3 className="text-sm font-rajdhani font-bold uppercase tracking-widest text-brass">Operation Stream</h3>
                            </div>
                            <div className="flex gap-2">
                                <button
                                    type="button"
                                    className="px-3 py-1.5 rounded border border-border bg-machine text-[10px] font-bold uppercase tracking-widest text-steel hover:border-cyan hover:text-cyan transition-colors"
                                    onClick={() => voxTransport.callTool('vox_rebalance', {})}
                                >
                                    REBALANCE
                                </button>
                                <button
                                    type="button"
                                    className="px-3 py-1.5 rounded border border-destructive bg-machine text-[10px] font-bold uppercase tracking-widest text-destructive hover:bg-destructive hover:text-white transition-colors"
                                    onClick={() => voxTransport.callTool('vox_emergency_stop', {})}
                                >
                                    ⛔ STOP ALL
                                </button>
                            </div>
                        </div>
                        
                        <div className="space-y-2 flex-1 overflow-y-auto pr-2 custom-scrollbar relative z-10">
                            {ops && ops.length > 0 ? ops.slice(0, 15).map((entry: any, idx: number) => (
                                <div 
                                    key={entry.id ?? entry.description ?? idx} 
                                    className="flex items-center justify-between p-3 rounded bg-machine border border-border hover:border-cyan hover:shadow-[inset_0_0_8px_var(--vox-cyan-glow)] transition-all"
                                >
                                    <div className="flex-1 min-w-0 pr-4">
                                        <div className="text-xs font-mono text-cyan truncate mb-1 uppercase tracking-wide">
                                            {entry.status === 'Doubted' && <span className="text-destructive font-bold mr-2">[SUSPECT]</span>}
                                            {entry.status === 'Validated' && <span className="text-primary font-bold mr-2">[VALIDATED]</span>}
                                            {entry.status === 'Overruled' && <span className="text-secondary font-bold mr-2">[OVERRULED]</span>}
                                            {entry.description || entry.op_type}
                                        </div>
                                        {entry.agent_id && (
                                            <div className="flex items-center gap-2 mt-1">
                                                <div className="text-[9px] font-mono text-steel uppercase tracking-widest">AGENT {entry.agent_id}</div>
                                                {entry.current_phase && (
                                                    <>
                                                        <span className="text-steel opacity-30">|</span>
                                                        <div className="text-[9px] font-bold font-mono text-cyan uppercase tracking-widest px-1.5 py-0.5 rounded bg-cyan bg-opacity-10 border border-cyan border-opacity-20">
                                                            PHASE: {entry.current_phase}
                                                        </div>
                                                    </>
                                                )}
                                                {entry.active_skill && (
                                                    <>
                                                        <span className="text-steel opacity-30">|</span>
                                                        <div className="text-[9px] font-bold font-mono text-primary uppercase tracking-widest px-1.5 py-0.5 rounded bg-primary bg-opacity-10 border border-primary border-opacity-20 flex items-center gap-1">
                                                            <Sparkles size={10} /> {entry.active_skill}
                                                        </div>
                                                    </>
                                                )}
                                            </div>
                                        )}
                                        {entry.audit_report && (
                                            <div className="mt-2 p-2 bg-void border border-border border-opacity-50 rounded text-[10px] font-mono text-steel italic">
                                                <span className="text-secondary opacity-70 uppercase font-bold mr-1">Audit:</span>
                                                {entry.audit_report}
                                            </div>
                                        )}
                                    </div>
                                    <div className="flex items-center gap-4 shrink-0">
                                        {entry.duration_ms && <span className="text-[10px] font-mono text-steel">{entry.duration_ms}ms</span>}
                                        {entry.status === 'Doubted' && (
                                            <button
                                                title="Overrule suspect flag and mark as completed"
                                                className="p-1.5 rounded border border-emerald-500 bg-machine text-emerald-500 hover:bg-emerald-500 hover:text-white transition-all transform hover:scale-110"
                                                onClick={() => {
                                                    if (entry.id) {
                                                        voxTransport.callTool('vox_overrule_task', { task_id: entry.id, reason: 'Human overrule from Dashboard' });
                                                    }
                                                }}
                                            >
                                                <CheckCircle2 size={14} />
                                            </button>
                                        )}
                                        {entry.status !== 'Doubted' && entry.status !== 'Validated' && entry.status !== 'Overruled' && (
                                            <button
                                                title="Flag this task as suspect for audit"
                                                className="p-1.5 rounded border border-destructive bg-machine text-destructive hover:bg-destructive hover:text-white transition-all transform hover:scale-110"
                                                onClick={() => {
                                                    if (entry.id) {
                                                        voxTransport.callTool('vox_doubt_task', { task_id: entry.id });
                                                    }
                                                }}
                                            >
                                                <AlertCircle size={14} />
                                            </button>
                                        )}
                                        <StateChip label={entry.status || "Completed"} tone={opRowTone(entry.status || "Completed")} />
                                    </div>
                                </div>
                            )) : (
                                <div className="flex items-center justify-center h-full text-[10px] font-mono uppercase tracking-widest text-steel">
                                    AWAITING TELEMETRY...
                                </div>
                            )}
                        </div>
                    </div>
                </div>
            )}

            {/* Right column: Pipeline Health & Attention */}
            <div className="col-span-4 flex flex-col gap-4">
                <div className="p-4 border border-border bg-surface rounded-xl shadow-lg">
                    <div className="flex items-center gap-2 mb-4 border-b border-border pb-3">
                        <MessageSquare size={16} className="text-secondary-foreground" />
                        <h3 className="text-sm font-rajdhani font-bold uppercase tracking-widest text-brass">Pipeline Health</h3>
                    </div>
                    <div className="flex flex-col items-center justify-center py-6">
                        {pipeline == null ? (
                            <div className="text-center text-[10px] font-mono text-steel uppercase tracking-widest">Awaiting Status</div>
                        ) : pipeline.ok === false ? (
                            <>
                                <AlertCircle size={32} className="text-destructive mb-3 animate-pulse" />
                                <div className="text-xs font-rajdhani font-bold uppercase tracking-widest text-destructive text-center">Pipeline Errors Detected</div>
                            </>
                        ) : (
                            <>
                                <CheckCircle2 size={32} className="text-primary mb-3 drop-shadow-[0_0_8px_var(--vox-amber-glow)]" />
                                <div className="text-xs font-rajdhani font-bold uppercase tracking-widest text-primary">Pipeline Veritas</div>
                            </>
                        )}
                    </div>
                </div>
                
                {attentionStatus?.enabled && (
                    <AttentionPanel status={attentionStatus} _alert={attentionAlert} />
                )}
            </div>
        </div>
    );
};
