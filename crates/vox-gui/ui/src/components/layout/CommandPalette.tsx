import React, { useState, useEffect } from 'react';
import { Icon } from '../ui/Icons';
import { CommandCatalogEntry } from '../../types/catalog';
import { Agent } from '../../types/dashboard';

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  onAction: (item: any) => void;
  agents: Agent[];
  skills: CommandCatalogEntry[];
}

export function CommandPalette({ open, onClose, onAction, agents, skills }: CommandPaletteProps) {
  const [q, setQ] = useState("");

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  const filteredAgents = agents.filter(a => 
    a.codename.toLowerCase().includes(q.toLowerCase()) || 
    a.id.toLowerCase().includes(q.toLowerCase())
  );

  const filteredSkills = skills.filter(s => 
    s.command.toLowerCase().includes(q.toLowerCase()) || 
    s.about.toLowerCase().includes(q.toLowerCase())
  );

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/60 backdrop-blur-sm pt-[14vh]" onClick={onClose}>
      <div className="w-[640px] max-w-[92vw] rounded-2xl border border-white/10 bg-zinc-950/90 shadow-[0_40px_120px_-30px_rgba(0,0,0,0.9)] backdrop-blur-2xl" onClick={e => e.stopPropagation()}>
        <div className="flex items-center gap-2 border-b border-white/5 px-4 py-3">
          <Icon.command className="size-4 text-brass"/>
          <input 
            autoFocus 
            value={q} 
            onChange={e => setQ(e.target.value)} 
            placeholder="Search commands, agents, or skills…" 
            className="flex-1 bg-transparent text-[14px] text-zinc-100 placeholder:text-zinc-600 outline-none"
          />
          <kbd className="rounded border border-white/10 bg-white/5 px-1.5 py-0.5 font-mono text-[10px] text-zinc-500">esc</kbd>
        </div>
        <div className="max-h-[380px] overflow-auto p-2 custom-scrollbar">
          {q.length === 0 && (
            <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-zinc-500 border-b border-white/5 mb-1">Quick Actions</div>
          )}
          
          {q.length === 0 && (
            <>
              <button onClick={() => { onAction({ id: 'submit' }); onClose(); }} className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left hover:bg-white/[0.04]">
                <span className="text-[13px] text-zinc-200">Submit new task…</span>
                <span className="font-mono text-[9px] uppercase tracking-widest text-zinc-500">loquela</span>
              </button>
            </>
          )}

          {filteredAgents.length > 0 && (
             <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-zinc-500 mt-2">Agents</div>
          )}
          {filteredAgents.map(a => (
            <button key={a.id} onClick={() => { onAction(a); onClose(); }} className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left hover:bg-white/[0.04]">
              <span className="text-[13px] text-zinc-200">{a.codename} ({a.id})</span>
              <span className="font-mono text-[9px] uppercase tracking-widest text-zinc-500">{a.phase}</span>
            </button>
          ))}

          {filteredSkills.length > 0 && (
             <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-zinc-500 mt-2">Skills</div>
          )}
          {filteredSkills.map(s => (
            <button key={s.command} onClick={() => { onAction(s); onClose(); }} className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left hover:bg-white/[0.04]">
              <div className="flex flex-col">
                <span className="text-[13px] text-zinc-200">{s.command}</span>
                <span className="text-[11px] text-zinc-500 truncate max-w-[400px]">{s.about}</span>
              </div>
              <span className="font-mono text-[9px] uppercase tracking-widest text-zinc-500">{s.tier}</span>
            </button>
          ))}

          {filteredAgents.length === 0 && filteredSkills.length === 0 && q.length > 0 && (
            <div className="px-3 py-6 text-center text-[12px] text-zinc-500">No matches found for "{q}"</div>
          )}
        </div>
      </div>
    </div>
  );
}
