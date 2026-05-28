import React, { useState } from 'react';
import { Search, Brackets, Database, Layers, ChevronRight, ChevronDown, FileCode, Terminal } from 'lucide-react';

const MOCK_AST = {
  kind: "Module",
  span: { start: 0, end: 1205 },
  items: [
    {
      kind: "FnDecl",
      name: "calculate_stats",
      params: [
        { name: "data", ty: "Collection[f64]" }
      ],
      return_ty: "Stats",
      span: { start: 24, end: 412 }
    },
    {
      kind: "ActorDecl",
      name: "FleetManager",
      members: [
        { kind: "Field", name: "agents", ty: "Map[AgentId, Agent]" },
        { kind: "Message", name: "dispatch", params: [{ name: "task", ty: "AgentTask" }] }
      ],
      span: { start: 420, end: 890 }
    }
  ]
};

export const AstView = () => {
  return (
    <div className="h-full flex flex-col bg-[#09090b]">
       <header className="px-10 py-8 border-b border-white/5 bg-white/[0.01]">
          <div className="flex items-center justify-between mb-6">
            <h2 className="text-3xl font-black text-white tracking-tight flex items-center gap-4">
               AST <span className="text-blue-500">Inspector</span>
               <div className="px-2 py-0.5 rounded bg-blue-500/10 border border-blue-500/20 text-[10px] font-bold uppercase text-blue-500 tracking-widest">Compiler V2</div>
            </h2>
            <div className="flex items-center gap-4">
               <div className="relative">
                 <Search size={14} className="absolute left-4 top-1/2 -translate-y-1/2 text-zinc-500" />
                 <input type="text" placeholder="Search AST nodes..." className="bg-white/5 border border-zinc-800 rounded-xl py-2 pl-12 pr-6 text-sm text-zinc-300 w-64 focus:border-blue-500/50 outline-none transition-all" />
               </div>
               <button className="px-5 py-2.5 rounded-xl bg-blue-600 font-bold text-[11px] uppercase tracking-widest text-white shadow-[0_0_20px_rgba(59,130,246,0.2)] hover:shadow-[0_0_30px_rgba(59,130,246,0.3)] transition-all">Reload Inspect</button>
            </div>
          </div>
          
          <div className="flex items-center gap-6">
             <StatMini label="Nodes" value="412" />
             <StatMini label="Max Depth" value="14" />
             <StatMini label="Resolution" value="99.2%" />
             <div className="h-4 w-px bg-zinc-800 ml-4 self-center" />
             <span className="text-[10px] font-mono text-zinc-500 uppercase tracking-widest">Active: crates/vox-ast/src/decl/fundecl.rs</span>
          </div>
       </header>

       <div className="flex-1 overflow-hidden grid grid-cols-12">
          {/* Tree View */}
          <div className="col-span-5 border-r border-white/5 overflow-y-auto p-8 space-y-4">
             <div className="space-y-1">
               <AstNode label="Module (0:1205)" icon={<FileCode size={14} />} expanded>
                  <AstNode label="Items (2 items)" icon={<Layers size={14} />} expanded>
                     <AstNode label="FnDecl: calculate_stats" icon={<Brackets size={14} />} active>
                         <AstNode label="Params (1 item)" icon={<Terminal size={14} />} />
                         <AstNode label="Body (Block)" icon={<Database size={14} />} />
                     </AstNode>
                     <AstNode label="ActorDecl: FleetManager" icon={<Database size={14} />} />
                  </AstNode>
               </AstNode>
             </div>
          </div>

          {/* Details Panel */}
          <div className="col-span-7 bg-white/[0.005] p-10 overflow-y-auto">
             <div className="mb-12">
                <span className="text-[11px] font-mono text-blue-500 mb-2 block uppercase tracking-widest">Node Properties</span>
                <h3 className="text-4xl font-black text-white/90 tracking-tighter mb-4">FnDecl <span className="text-white/20">calculate_stats</span></h3>
                <div className="flex gap-3">
                   <Tag label="Method: GET" />
                   <Tag label="Visibility: Public" />
                   <Tag label="Pure: Yes" />
                </div>
             </div>

             <div className="glass rounded-[2rem] border border-white/5 p-8 font-mono text-xs leading-relaxed text-zinc-400">
                <pre>{JSON.stringify(MOCK_AST.items[0], null, 2)}</pre>
             </div>
          </div>
       </div>
    </div>
  );
};

const AstNode = ({ label, icon, expanded, active, children }: any) => (
  <div className="flex flex-col">
    <div className={`flex items-center gap-3 py-2 px-3 rounded-xl cursor-default transition-all ${
      active ? 'bg-blue-600/10 text-blue-500 border border-blue-500/20' : 'text-zinc-500 hover:bg-white/5 hover:text-zinc-300'
    }`}>
      {children ? (expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />) : <div className="w-3.5" />}
      <div className={`w-8 h-8 rounded-lg flex items-center justify-center border ${
        active ? 'bg-blue-500/10 border-blue-500/30 text-blue-500' : 'bg-zinc-900 border-white/5 text-zinc-600'
      }`}>
        {icon}
      </div>
      <span className={`text-[13px] font-medium tracking-tight ${active ? 'text-white/90' : ''}`}>{label}</span>
    </div>
    {children && expanded && (
      <div className="pl-8 border-l border-zinc-800 ml-4.5 mt-1 space-y-1 py-1">
        {children}
      </div>
    )}
  </div>
)

const StatMini = ({ label, value }: any) => (
  <div className="flex items-center gap-2">
    <span className="text-[10px] font-bold text-zinc-600 uppercase tracking-widest">{label}</span>
    <span className="text-sm font-bold text-zinc-300 font-mono">{value}</span>
  </div>
)

const Tag = ({ label }: any) => (
  <span className="px-3 py-1 rounded-full bg-white/[0.03] border border-white/10 text-[10px] font-bold text-zinc-400 uppercase tracking-widest">{label}</span>
)
