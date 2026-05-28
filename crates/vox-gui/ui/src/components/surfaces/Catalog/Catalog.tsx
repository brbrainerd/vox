import React, { useState } from 'react';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';

const SKILL_GLYPHS: Record<string, (s: any) => React.ReactNode> = {
  tdd: (s) => <svg viewBox="0 0 64 64" {...s}><defs><linearGradient id="gtdd" x1="0" x2="1"><stop offset="0" stopColor="#22d3ee"/><stop offset="1" stopColor="#a78bfa"/></linearGradient></defs><circle cx="32" cy="32" r="22" fill="none" stroke="url(#gtdd)" strokeWidth="1.2"/><path d="M22 32l7 7 13-14" stroke="url(#gtdd)" strokeWidth="2" fill="none" strokeLinecap="round" strokeLinejoin="round"/><circle cx="32" cy="32" r="29" fill="none" stroke="url(#gtdd)" strokeOpacity="0.25" strokeDasharray="3 5"/></svg>,
  refactor: (s) => <svg viewBox="0 0 64 64" {...s}><defs><linearGradient id="gref" x1="0" x2="1"><stop offset="0" stopColor="#d4af37"/><stop offset="1" stopColor="#22d3ee"/></linearGradient></defs><path d="M16 22h22a8 8 0 1 1 0 16H22" fill="none" stroke="url(#gref)" strokeWidth="1.5"/><path d="m16 26-4-4 4-4M22 34l4 4-4 4" fill="none" stroke="url(#gref)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/></svg>,
  research: (s) => <svg viewBox="0 0 64 64" {...s}><defs><linearGradient id="gres" x1="0" x2="1"><stop offset="0" stopColor="#22d3ee"/><stop offset="1" stopColor="#d4af37"/></linearGradient></defs><circle cx="28" cy="28" r="14" fill="none" stroke="url(#gres)" strokeWidth="1.5"/><path d="m38 38 12 12" stroke="url(#gres)" strokeWidth="1.8" strokeLinecap="round"/><path d="M20 28h16M28 20v16" stroke="url(#gres)" strokeWidth="1" opacity="0.5"/></svg>,
};

const TIER_MAP: Record<string, any> = {
  recommended:   { label: "Mature", ring: "ring-emerald-400/20", text: "text-emerald-300", chip: "bg-emerald-400/5 border-emerald-400/20" },
  advanced:      { label: "Stable", ring: "ring-cyan-400/20",    text: "text-cyan-300",    chip: "bg-cyan-400/5 border-cyan-400/20" },
  feature_gated: { label: "Preview", ring: "ring-brass/25",       text: "text-brass",       chip: "bg-brass/5 border-brass/25" },
};

function SkillCard({ skill, onDeploy, pending }: any) {
  const tier = TIER_MAP[skill.tier] || TIER_MAP.advanced;
  const Glyph = SKILL_GLYPHS.tdd; // Fallback
  const name = skill.name || skill.command.replace('vox ', '');
  const desc = skill.desc || skill.about;
  const id = skill.id || skill.command;

  return (
    <div className={`group relative overflow-hidden rounded-2xl border border-white/[0.06] bg-zinc-950/40 p-4 transition hover:border-white/15 hover:-translate-y-[2px] ring-1 ${tier.ring}`}>
      <div className="flex items-start justify-between">
        <div className={`relative size-14 rounded-xl bg-zinc-950 ring-1 ring-white/10 ${tier.text}`}>
          <Glyph className="absolute inset-0 size-full p-2.5" />
        </div>
        <span className={`rounded-full border px-2 py-0.5 font-display text-[10px] uppercase tracking-[0.18em] ${tier.chip} ${tier.text}`}>{tier.label}</span>
      </div>
      <div className="mt-3">
        <div className="font-display text-[16px] font-semibold tracking-tight text-zinc-100 uppercase truncate">{name}</div>
        <div className="mt-1 text-[12px] leading-relaxed text-zinc-400 h-12 overflow-hidden">{desc}</div>
      </div>
      <div className="mt-4 flex items-center justify-end">
        <button
          onClick={() => onDeploy(skill)}
          disabled={pending}
          className={`group/btn flex items-center gap-1.5 rounded-md border px-3 py-1.5 font-display text-[11px] uppercase tracking-[0.18em] transition ${pending ? "border-emerald-400/40 bg-emerald-400/10 text-emerald-300" : "border-brass/30 bg-brass/[0.06] text-brass hover:bg-brass/15 hover:border-brass/60"}`}
        >
          {pending ? <><Icon.check className="size-3.5"/>Dispatched</> : <><Icon.bolt className="size-3.5 transition group-hover/btn:translate-x-[1px]"/>Deploy</>}
        </button>
      </div>
    </div>
  );
}

export function Catalog({ skills = [], onDeploy, deployedSet }: any) {
  const [q, setQ] = useState("");
  const [tier, setTier] = useState("All");
  const tiers = ["All", "Mature", "Stable", "Preview"];
  
  const filtered = (skills || []).filter((s: any) => {
    const t = TIER_MAP[s.tier] || TIER_MAP.advanced;
    const name = s.name || s.command || "";
    const desc = s.desc || s.about || "";
    return (tier === "All" || t.label === tier) &&
    (q.trim() === "" || name.toLowerCase().includes(q.toLowerCase()) || desc.toLowerCase().includes(q.toLowerCase()));
  });

  return (
    <div className="flex flex-col gap-5 p-5">
      <Glass className="p-5">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Skills</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">First-party capabilities · command registry</p>
          </div>
          <div className="flex items-center gap-3">
            <div className="flex items-center gap-2 rounded-lg border border-white/5 bg-white/[0.02] px-3 py-1.5">
              <Icon.search className="size-3.5 text-zinc-500" />
              <input value={q} onChange={e => setQ(e.target.value)} placeholder="Search skills…" className="w-48 bg-transparent text-[12px] text-zinc-200 placeholder:text-zinc-600 outline-none" />
            </div>
            <div className="flex gap-1 rounded-lg border border-white/5 bg-white/[0.02] p-1">
              {tiers.map(t => (
                <button key={t} onClick={() => setTier(t)} className={`rounded-md px-3 py-1 font-display text-[10px] uppercase tracking-[0.18em] transition ${tier === t ? "bg-white/10 text-zinc-100" : "text-zinc-500 hover:text-zinc-300"}`}>{t}</button>
              ))}
            </div>
          </div>
        </div>
      </Glass>
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
        {filtered.map((s: any) => <SkillCard key={s.id || s.command} skill={s} onDeploy={onDeploy} pending={deployedSet.has(s.id || s.command)} />)}
      </div>
    </div>
  );
}
