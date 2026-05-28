import React from 'react';
import { Layers, Database, Code, Brackets, CheckCircle, AlertCircle, Info } from 'lucide-react';

const STAGES = [
  { id: 'lexer', name: 'Lexer', icon: <Layers size={18} />, desc: 'Logos-based tokenization' },
  { id: 'parser', name: 'Parser', icon: <Database size={18} />, desc: 'Rowan GreenTree CST generation' },
  { id: 'hir', name: 'HIR', icon: <Brackets size={18} />, desc: 'High-level IR with name resolution' },
  { id: 'typeck', name: 'TypeCheck', icon: <CheckCircle size={18} />, desc: 'Bidirectional unification logic' },
  { id: 'codegen', name: 'CodeGen', icon: <Code size={18} />, desc: 'Rust backend + TS (`routes.manifest.ts`, `vox-client.ts`, components)' }
];

export const PipelineView = ({ status = {} }: any) => {
  return (
    <div className="h-full grid grid-cols-5 divide-x divide-white/5 bg-[#09090b]">
       {STAGES.map((stage, idx) => {
         const isOk = status[stage.id] === 'ok' || !status[stage.id]; // fallback for demo
         return (
           <div key={stage.id} className={`p-8 flex flex-col group hover:bg-white/[0.01] transition-all relative overflow-hidden`}>
             {/* Stage Progress */}
             <div className="flex items-center justify-between mb-10 z-10">
                <div className="w-10 h-10 rounded-xl bg-zinc-900 border border-white/5 flex items-center justify-center text-zinc-500 group-hover:text-blue-500 group-hover:border-blue-500/30 transition-all duration-500">
                  {stage.icon}
                </div>
                {isOk ? (
                   <span className="text-[10px] font-bold text-emerald-500 uppercase tracking-widest bg-emerald-500/10 px-2 py-0.5 rounded border border-emerald-500/20">Operational</span>
                ) : (
                   <span className="text-[10px] font-bold text-rose-500 uppercase tracking-widest bg-rose-500/10 px-2 py-0.5 rounded border border-rose-500/20">Fault Detected</span>
                )}
             </div>

             <div className="relative z-10">
               <h3 className="text-2xl font-bold text-white/90 mb-1 group-hover:text-white transition-colors">{stage.name}</h3>
               <p className="text-zinc-500 text-sm leading-relaxed mb-12">{stage.desc}</p>
             </div>

             {/* Dynamic Log Feed */}
             <div className="flex-1 glass rounded-2xl border border-white/5 p-5 font-mono text-[11px] overflow-hidden group-hover:border-blue-500/20 transition-all">
                <div className="flex items-center gap-2 mb-4">
                  <div className="w-2 h-2 rounded-full bg-emerald-500" />
                  <span className="text-[9px] font-bold text-zinc-500 uppercase">Live Output</span>
                </div>
                <div className="space-y-1.5 opacity-60">
                   <div className="flex gap-2">
                     <span className="text-zinc-600">[{idx % 2 === 0 ? 'TRACE' : 'DEBUG'}]</span>
                     <span className="text-zinc-300">Initialized {stage.id} runtime context...</span>
                   </div>
                   <div className="flex gap-2">
                     <span className="text-zinc-600">[META]</span>
                     <span className="text-zinc-400">Loading worker pool (8 threads)</span>
                   </div>
                   <div className="flex gap-2">
                     <span className="text-emerald-600">[DONE]</span>
                     <span className="text-emerald-400/80">Calibration matrix validated.</span>
                   </div>
                   <div className="mt-4 animate-pulse">
                     <span className="text-blue-500">> STAGE {idx + 1} ACTIVE</span>
                   </div>
                </div>
             </div>

             {/* Background ID decoration */}
             <span className="absolute -bottom-10 -right-4 text-[120px] font-bold text-white/[0.02] -z-0 pointer-events-none select-none">0{idx + 1}</span>
           </div>
         );
       })}
    </div>
  );
};
