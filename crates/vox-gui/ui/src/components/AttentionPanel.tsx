import React, { useState } from 'react';
import { Zap, Activity, Settings2 } from 'lucide-react';
import { voxTransport } from '../transport';
import type { AttentionAlert, AttentionStatusPayload } from '../types';

export const AttentionPanel = ({
    status,
    _alert,
}: {
    status: AttentionStatusPayload | null;
    _alert: AttentionAlert | null;
}) => {
    const [expanded, setExpanded] = useState(false);
    const [advanced, setAdvanced] = useState(false);

    if (!status) return null;

    const maxMs = status.max_ms ?? 3600000;
    const spentMs = status.spent_ms ?? 0;
    const ratio = Math.min(1, Math.max(0, spentMs / maxMs));
    const remainsMs = Math.max(0, maxMs - spentMs);

    const formatTime = (ms: number) => {
        const h = Math.floor(ms / 3600000);
        const m = Math.floor((ms % 3600000) / 60000);
        return h > 0 ? `${h}h ${m}m` : `${m}m`;
    };

    const isExhausted = !!status.exhausted || ratio >= 1.0;
    const isAlert = ratio >= (status.alert_threshold ?? 0.85);

    const arcColor = isExhausted ? '#EF4444' : isAlert ? '#F59E0B' : '#10B981';
    
    const radius = 40;
    const circumference = 2 * Math.PI * radius;
    const strokeDashoffset = circumference - (ratio * circumference);

    return (
        <div className="p-4 border border-border bg-surface rounded-xl shadow-lg mt-4 w-full">
            <div className="flex items-center justify-between mb-4 border-b border-border pb-3">
                <div className="flex items-center gap-2">
                    <Zap size={16} className="text-secondary-foreground" />
                    <h3 className="text-sm font-rajdhani font-bold uppercase tracking-widest text-brass">Attention Budget</h3>
                </div>
                <div className="flex gap-2">
                    <div className={`px-2 py-0.5 rounded text-[10px] font-bold uppercase tracking-widest border flex items-center gap-1
                        ${status.focus_depth === 'Deep' ? 'bg-destructive/10 border-destructive text-destructive animate-pulse' : 
                          status.focus_depth === 'Focused' ? 'bg-amber-500/10 border-amber-500/30 text-amber-500' : 
                          'bg-cyan/10 border-cyan/30 text-cyan'}`}>
                        {status.focus_depth ?? 'Ambient'}
                    </div>
                </div>
            </div>

            <div className="grid grid-cols-12 gap-4">
                <div className="col-span-4 flex flex-col items-center justify-center">
                    <div className="relative w-24 h-24">
                        <svg className="w-full h-full transform -rotate-90" viewBox="0 0 100 100">
                            <circle cx="50" cy="50" r={radius} fill="none" stroke="var(--vscode-editorWidget-border)" strokeWidth="8" />
                            <circle 
                                cx="50" 
                                cy="50" 
                                r={radius} 
                                fill="none" 
                                stroke={arcColor} 
                                strokeWidth="8" 
                                strokeDasharray={circumference}
                                strokeDashoffset={strokeDashoffset}
                                strokeLinecap="round"
                                className="transition-all duration-1000 ease-out"
                            />
                        </svg>
                        <div className="absolute inset-0 flex flex-col items-center justify-center">
                            <span className="text-[12px] font-bold text-foreground font-mono">{(ratio * 100).toFixed(0)}%</span>
                        </div>
                    </div>
                    <div className="mt-2 text-[10px] text-steel font-mono">
                        ~{formatTime(remainsMs)} left
                    </div>
                </div>

                <div className="col-span-8 flex flex-col gap-3 justify-center">
                    <div className="flex items-center justify-between">
                        <span className="text-[10px] text-steel uppercase tracking-widest">Interruptions</span>
                        <div className="text-[11px] font-mono text-cyan flex items-center gap-1">
                            <Activity size={10} /> {(status.interrupt_freq_per_hour ?? 0).toFixed(1)}/hr
                        </div>
                    </div>
                    
                    <div className="flex items-center justify-between">
                        <span className="text-[10px] text-steel uppercase tracking-widest">Autonomy</span>
                        <div className="w-24 bg-void border border-border h-2 rounded overflow-hidden">
                            <div className="bg-cyan h-full" style={{ width: `${(status.auto_approve_ratio ?? 0) * 100}%` }} />
                        </div>
                    </div>

                    {(status.inbox_suppressed_count ?? 0) > 0 && (
                        <div className="text-[10px] text-destructive bg-destructive/10 border border-destructive/20 p-1.5 rounded text-center uppercase tracking-widest font-bold">
                            {status.inbox_suppressed_count} Messages suppressed (Deep Focus)
                        </div>
                    )}
                </div>
            </div>

            <div className="mt-4 pt-3 border-t border-border">
                <button 
                    onClick={() => setExpanded(!expanded)} 
                    className="w-full flex items-center justify-between text-[10px] text-steel hover:text-cyan uppercase tracking-widest font-bold"
                >
                    <span className="flex items-center gap-1"><Settings2 size={12} /> Configuration</span>
                    <span>{expanded ? '▲' : '▼'}</span>
                </button>
                
                {expanded && (
                    <div className="mt-3 space-y-4">
                        <div className="flex items-center justify-between">
                            <label className="text-[10px] text-steel uppercase tracking-widest">Attention Budgeting</label>
                            <input 
                                type="checkbox" 
                                checked={status.enabled ?? false} 
                                onChange={(e) => voxTransport.callTool('vox_preference_set', { key: 'attention.enabled', value: e.target.checked ? 'true' : 'false' })}
                                className="accent-cyan"
                            />
                        </div>

                        <div>
                            <label className="text-[10px] text-steel uppercase tracking-widest flex justify-between mb-1">
                                <span>Budget Cap</span>
                                <span className="font-mono text-cyan">{formatTime(maxMs)}</span>
                            </label>
                            <input 
                                type="range" 
                                min={3600000} 
                                max={86400000} 
                                step={1800000} 
                                value={maxMs} 
                                onChange={(e) => voxTransport.callTool('vox_preference_set', { key: 'attention.budget_ms', value: String(e.target.value) })}
                                className="w-full accent-cyan"
                            />
                        </div>

                        <div>
                            <label className="text-[10px] text-steel uppercase tracking-widest flex justify-between mb-1">
                                <span>Alert Threshold</span>
                                <span className="font-mono text-cyan">{((status.alert_threshold ?? 0.85) * 100).toFixed(0)}%</span>
                            </label>
                            <input 
                                type="range" 
                                min={0.5} 
                                max={0.95} 
                                step={0.05} 
                                value={status.alert_threshold ?? 0.85} 
                                onChange={(e) => voxTransport.callTool('vox_preference_set', { key: 'attention.alert_threshold', value: String(e.target.value) })}
                                className="w-full accent-cyan"
                            />
                        </div>

                        <div className="border border-border rounded p-2 bg-void">
                            <button 
                                onClick={() => setAdvanced(!advanced)}
                                className="text-[9px] text-steel hover:text-cyan uppercase tracking-widest mb-2"
                            >
                                {advanced ? '- Advanced Policy' : '+ Advanced Policy'}
                            </button>
                            {advanced && (
                                <div className="space-y-3 mt-2">
                                    <div>
                                        <label className="text-[9px] text-steel uppercase flex justify-between mb-1">
                                            Interrupt Cost (ms)
                                        </label>
                                        <div className="flex gap-2">
                                            <input 
                                                id="cost-input"
                                                type="number" 
                                                defaultValue={1395000} 
                                                className="w-full text-[10px] bg-machine border border-border px-2 py-1 rounded"
                                            />
                                            <button 
                                                onClick={() => {
                                                    const val = (document.getElementById('cost-input') as HTMLInputElement).value;
                                                    console.warn('Terminal command execution not yet supported in standalone mode:', `vox attention config set cost ${val}`);
                                                }}
                                                className="px-2 py-1 bg-machine border border-border text-[9px] text-cyan hover:bg-cyan/10 rounded"
                                            >
                                                Apply
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            )}
                        </div>

                        <div className="pt-2 border-t border-border flex justify-between items-center">
                            <select 
                                className="bg-machine border border-border text-[9px] text-steel p-1 rounded"
                                onChange={(e) => {
                                    if (e.target.value) {
                                        let score = 1.0;
                                        if (e.target.value === 'Monitored') score = 0.5;
                                        if (e.target.value === 'Untrusted') score = 0.0;
                                        voxTransport.callTool('vox_trust_override', { agent_id: 0, trust_score: score });
                                        e.target.value = "";
                                    }
                                }}
                            >
                                <option value="">Override Trust Tier...</option>
                                <option value="Trusted">Trusted</option>
                                <option value="Monitored">Monitored</option>
                                <option value="Untrusted">Untrusted</option>
                            </select>
                            <button 
                                onClick={() => {
                                    if (confirm('Reset attention session?')) {
                                        voxTransport.callTool('vox_attention_reset', {});
                                    }
                                }}
                                className="px-3 py-1.5 text-[10px] text-destructive border border-destructive/50 bg-destructive/10 rounded hover:bg-destructive hover:text-white transition-colors"
                            >
                                RESET SESSION
                            </button>
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
};
