import React, { useState } from 'react';
import { motion } from 'framer-motion';
import { AlertTriangle, BrainCircuit, Target, CheckCircle, XCircle, Shield, GitBranch } from 'lucide-react';
import { voxTransport } from '../transport';

export function IntentionMatrix({ intents, socratesStatus }: any) {
    const [enforceGate, setEnforceGate] = useState(true);
    const [selectedIntent, setSelectedIntent] = useState<any>(null);

    const matrix = intents || [];
    const shadowRisk = socratesStatus?.shadowRisk || 0;

    const toggleGate = async () => {
        const newValue = !enforceGate;
        setEnforceGate(newValue);
        await voxTransport.callTool('vox_preference_set', {
            user_id: "default",
            key: 'socrates_gate_enforced',
            value: newValue.toString()
        });
    };

    return (
        <div className="p-4 bg-background h-full overflow-y-auto w-full flex flex-col gap-6 text-white pb-20">
            <div className="flex justify-between items-center bg-white/[0.02] border border-white/5 p-6 rounded-3xl glass">
                <div>
                    <h2 className="text-3xl font-black tracking-tighter uppercase mb-2 flex items-center gap-3">
                        <BrainCircuit size={28} className="text-violet-500" /> 
                        Intention <span className="text-violet-500">Matrix</span>
                    </h2>
                    <p className="text-xs text-zinc-400 font-bold tracking-widest uppercase">Socrates Protocol Agent Evaluation</p>
                </div>
                
                <div className="flex items-center gap-6">
                    {shadowRisk > 0.4 && (
                        <div className="flex items-center gap-2 bg-rose-500/20 text-rose-500 px-4 py-2 rounded-full border border-rose-500/50 animate-pulse">
                            <AlertTriangle size={16} />
                            <span className="font-bold text-[10px] uppercase tracking-widest">Hallucination Risk Detected</span>
                        </div>
                    )}
                    
                    <button 
                        onClick={toggleGate}
                        className={`flex items-center gap-2 px-4 py-2 rounded-xl text-[10px] font-bold uppercase tracking-widest transition-all border ${
                            enforceGate 
                            ? 'bg-emerald-500/20 text-emerald-500 border-emerald-500/50 shadow-[0_0_20px_rgba(16,185,129,0.2)]' 
                            : 'bg-zinc-500/20 text-zinc-500 border-zinc-500/50'
                        }`}
                    >
                        <Shield size={14} />
                        {enforceGate ? 'Socrates Gate: Enforced' : 'Socrates Gate: Shadowed'}
                    </button>
                </div>
            </div>

            <div className="grid grid-cols-3 gap-6 flex-1">
                <div className="col-span-2 glass rounded-3xl border border-white/5 p-6 flex flex-col">
                    <h3 className="text-xs font-bold text-zinc-500 uppercase tracking-widest mb-6 flex items-center gap-2">
                        <Target size={14} /> Agent Goals vs Confidence
                    </h3>
                    
                    <div className="grid grid-cols-4 gap-4 flex-1">
                        {matrix.length === 0 ? (
                            <div className="col-span-4 flex items-center justify-center text-zinc-500 text-xs font-bold uppercase tracking-widest border border-dashed border-white/10 rounded-xl">
                                No intentions received
                            </div>
                        ) : matrix.map((intent: any) => (
                            <motion.div 
                                key={intent.id}
                                onClick={() => setSelectedIntent(intent)}
                                className={`
                                    relative p-4 rounded-xl border flex flex-col justify-between cursor-pointer transition-all hover:scale-[1.02]
                                    ${intent.active ? 'bg-violet-500/10 border-violet-500/50 glow-violet' : 'bg-black/40 border-white/5 hover:border-white/20'}
                                    ${intent.confidence < 0.5 ? 'border-b-4 border-b-rose-500/50' : 'border-b-4 border-b-emerald-500/50'}
                                `}
                            >
                                {intent.active && (
                                    <div className="absolute inset-0 rounded-xl bg-violet-400/5 animate-pulse pointer-events-none" />
                                )}
                                <div className="text-sm font-bold leading-tight mb-4 z-10">{intent.goal}</div>
                                <div className="flex justify-between items-end z-10">
                                    <div className="flex flex-col">
                                        <span className="text-[9px] text-zinc-500 uppercase font-bold tracking-widest mb-1">Confidence</span>
                                        <span className={`text-xl font-black ${intent.confidence >= 0.5 ? 'text-emerald-500' : 'text-rose-500'}`}>
                                            {(intent.confidence * 100).toFixed(0)}%
                                        </span>
                                    </div>
                                    <div className="text-zinc-600">
                                        {intent.confidence >= 0.5 ? <CheckCircle size={18} className="text-emerald-500/50" /> : <XCircle size={18} className="text-rose-500/50" />}
                                    </div>
                                </div>
                            </motion.div>
                        ))}
                    </div>
                </div>

                <div className="col-span-1 glass rounded-3xl border border-white/5 p-6 flex flex-col gap-6 overflow-hidden">
                    {selectedIntent ? (
                        <>
                            <div>
                                <h3 className="text-xs font-bold text-violet-400 uppercase tracking-widest mb-2 flex items-center gap-2">
                                    <GitBranch size={14} /> Speculative Branch
                                </h3>
                                <div className="text-sm font-mono bg-black/50 p-3 rounded-xl border border-white/5 text-zinc-300">
                                    {selectedIntent.branch}
                                </div>
                            </div>
                            
                            <div>
                                <h3 className="text-xs font-bold text-zinc-500 uppercase tracking-widest mb-2">Internal Prompt Trace</h3>
                                <div className="text-[10px] font-mono bg-black/50 p-4 rounded-xl border border-white/5 text-zinc-400 leading-relaxed max-h-40 overflow-y-auto">
                                    {selectedIntent.prompt_trace || "-- no trace available --"}
                                </div>
                            </div>

                            <div className="mt-auto pt-4 border-t border-white/5 flex items-center justify-between">
                                <div className="flex flex-col">
                                    <span className="text-[9px] text-zinc-500 uppercase font-bold tracking-widest mb-1">Agent Reliability Score</span>
                                    <span className="text-lg font-black text-blue-400">
                                        {(selectedIntent.reliable * 100).toFixed(1)}
                                    </span>
                                </div>
                                {selectedIntent.confidence < 0.5 && enforceGate && (
                                    <button 
                                        onClick={() => voxTransport.callTool('vox_fail_task', { task_id: String(selectedIntent.id) })}
                                        className="px-3 py-1.5 bg-rose-500/10 text-rose-500 border border-rose-500/30 rounded-lg text-[10px] font-bold uppercase hover:bg-rose-500/20 transition-all"
                                    >
                                        Reject Execution
                                    </button>
                                )}
                            </div>
                        </>
                    ) : (
                        <div className="h-full flex flex-col items-center justify-center text-center opacity-50">
                            <BrainCircuit size={32} className="text-zinc-500 mb-4" />
                            <div className="text-xs font-bold uppercase tracking-widest text-zinc-400">Select an intention cell<br/>to view execution details</div>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
