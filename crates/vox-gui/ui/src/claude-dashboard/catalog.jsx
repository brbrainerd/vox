// Superpowers Catalog — skill vault cards with Deploy action

const SKILL_GLYPHS = {
  tdd: (s) => <svg viewBox="0 0 64 64" {...s}><defs><linearGradient id="gtdd" x1="0" x2="1"><stop offset="0" stopColor="#22d3ee"/><stop offset="1" stopColor="#a78bfa"/></linearGradient></defs><circle cx="32" cy="32" r="22" fill="none" stroke="url(#gtdd)" strokeWidth="1.2"/><path d="M22 32l7 7 13-14" stroke="url(#gtdd)" strokeWidth="2" fill="none" strokeLinecap="round" strokeLinejoin="round"/><circle cx="32" cy="32" r="29" fill="none" stroke="url(#gtdd)" strokeOpacity="0.25" strokeDasharray="3 5"/></svg>,
  refactor: (s) => <svg viewBox="0 0 64 64" {...s}><defs><linearGradient id="gref" x1="0" x2="1"><stop offset="0" stopColor="#d4af37"/><stop offset="1" stopColor="#22d3ee"/></linearGradient></defs><path d="M16 22h22a8 8 0 1 1 0 16H22" fill="none" stroke="url(#gref)" strokeWidth="1.5"/><path d="m16 26-4-4 4-4M22 34l4 4-4 4" fill="none" stroke="url(#gref)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/></svg>,
  research: (s) => <svg viewBox="0 0 64 64" {...s}><defs><linearGradient id="gres" x1="0" x2="1"><stop offset="0" stopColor="#22d3ee"/><stop offset="1" stopColor="#d4af37"/></linearGradient></defs><circle cx="28" cy="28" r="14" fill="none" stroke="url(#gres)" strokeWidth="1.5"/><path d="m38 38 12 12" stroke="url(#gres)" strokeWidth="1.8" strokeLinecap="round"/><path d="M20 28h16M28 20v16" stroke="url(#gres)" strokeWidth="1" opacity="0.5"/></svg>,
  writing: (s) => <svg viewBox="0 0 64 64" {...s}><defs><linearGradient id="gwr" x1="0" x2="1"><stop offset="0" stopColor="#a78bfa"/><stop offset="1" stopColor="#22d3ee"/></linearGradient></defs><path d="M14 50l30-30 6 6-30 30-8 2 2-8Z" fill="none" stroke="url(#gwr)" strokeWidth="1.5" strokeLinejoin="round"/><path d="m38 26 6 6" stroke="url(#gwr)" strokeWidth="1.5"/></svg>,
  memory: (s) => <svg viewBox="0 0 64 64" {...s}><defs><linearGradient id="gmem" x1="0" x2="1"><stop offset="0" stopColor="#d4af37"/><stop offset="1" stopColor="#a78bfa"/></linearGradient></defs><rect x="14" y="22" width="36" height="22" rx="3" fill="none" stroke="url(#gmem)" strokeWidth="1.5"/><path d="M22 22v22M30 22v22M38 22v22M46 22v22" stroke="url(#gmem)" strokeWidth="1" opacity="0.6"/><path d="M22 18v4M30 18v4M38 18v4M46 18v4M22 44v4M30 44v4M38 44v4M46 44v4" stroke="url(#gmem)" strokeWidth="1.2"/></svg>,
  triage: (s) => <svg viewBox="0 0 64 64" {...s}><defs><linearGradient id="gtri" x1="0" x2="1"><stop offset="0" stopColor="#fbbf24"/><stop offset="1" stopColor="#a78bfa"/></linearGradient></defs><path d="M32 12 8 52h48L32 12Z" fill="none" stroke="url(#gtri)" strokeWidth="1.5" strokeLinejoin="round"/><path d="M32 28v12M32 44v2" stroke="url(#gtri)" strokeWidth="2" strokeLinecap="round"/></svg>,
  crypto: (s) => <svg viewBox="0 0 64 64" {...s}><defs><linearGradient id="gcr" x1="0" x2="1"><stop offset="0" stopColor="#a78bfa"/><stop offset="1" stopColor="#d4af37"/></linearGradient></defs><rect x="18" y="28" width="28" height="22" rx="3" fill="none" stroke="url(#gcr)" strokeWidth="1.5"/><path d="M24 28v-6a8 8 0 1 1 16 0v6" fill="none" stroke="url(#gcr)" strokeWidth="1.5"/><circle cx="32" cy="39" r="2.5" fill="url(#gcr)"/></svg>,
  orchestrate: (s) => <svg viewBox="0 0 64 64" {...s}><defs><linearGradient id="gor" x1="0" x2="1"><stop offset="0" stopColor="#22d3ee"/><stop offset="1" stopColor="#d4af37"/></linearGradient></defs><circle cx="32" cy="32" r="6" fill="none" stroke="url(#gor)" strokeWidth="1.5"/><circle cx="12" cy="32" r="4" fill="none" stroke="url(#gor)" strokeWidth="1.3"/><circle cx="52" cy="32" r="4" fill="none" stroke="url(#gor)" strokeWidth="1.3"/><circle cx="32" cy="12" r="4" fill="none" stroke="url(#gor)" strokeWidth="1.3"/><circle cx="32" cy="52" r="4" fill="none" stroke="url(#gor)" strokeWidth="1.3"/><path d="M18 32h8M38 32h10M32 18v8M32 38v10" stroke="url(#gor)" strokeWidth="1.2"/></svg>,
};

const TIER_COLOR = {
  Mature:   { ring: "ring-emerald-400/20", text: "text-emerald-300", chip: "bg-emerald-400/5 border-emerald-400/20" },
  Stable:   { ring: "ring-cyan-400/20",    text: "text-cyan-300",    chip: "bg-cyan-400/5 border-cyan-400/20" },
  Preview:  { ring: "ring-brass/25",       text: "text-brass",       chip: "bg-brass/5 border-brass/25" },
  Emergent: { ring: "ring-violet-400/20",  text: "text-violet-300",  chip: "bg-violet-400/5 border-violet-400/20" },
};

function SkillCard({ skill, onDeploy, pending }) {
  const tier = TIER_COLOR[skill.tier] || TIER_COLOR.Stable;
  const Glyph = SKILL_GLYPHS[skill.glyph] || SKILL_GLYPHS.tdd;
  return (
    <div className={`group relative overflow-hidden rounded-2xl border border-white/[0.06] bg-zinc-950/40 p-4 transition hover:border-white/15 hover:-translate-y-[2px] ring-1 ${tier.ring}`}>
      {/* corner bracket marks */}
      <div className="pointer-events-none absolute inset-0">
        <span className="absolute top-2 left-2 size-3 border-l border-t border-white/15" />
        <span className="absolute top-2 right-2 size-3 border-r border-t border-white/15" />
        <span className="absolute bottom-2 left-2 size-3 border-l border-b border-white/15" />
        <span className="absolute bottom-2 right-2 size-3 border-r border-b border-white/15" />
      </div>
      <div className="pointer-events-none absolute -inset-px rounded-2xl opacity-0 transition group-hover:opacity-100 [background:radial-gradient(220px_140px_at_var(--mx,50%)_var(--my,0%),rgba(212,175,55,0.10),transparent_70%)]" />

      <div className="flex items-start justify-between">
        <div className={`relative size-14 rounded-xl bg-zinc-950 ring-1 ring-white/10 ${tier.text}`}>
          <Glyph className="absolute inset-0 size-full p-2.5" />
          <div className="pointer-events-none absolute inset-0 rounded-xl shadow-[inset_0_0_20px_rgba(255,255,255,0.04)]" />
        </div>
        <span className={`rounded-full border px-2 py-0.5 font-display text-[10px] uppercase tracking-[0.18em] ${tier.chip} ${tier.text}`}>{skill.tier}</span>
      </div>

      <div className="mt-3">
        <div className="font-display text-[16px] font-semibold tracking-tight text-zinc-100">{skill.name}</div>
        <div className="mt-1 text-[12px] leading-relaxed text-zinc-400">{skill.desc}</div>
      </div>

      <div className="mt-4 flex items-center justify-between">
        <div className="flex items-center gap-3 font-mono text-[10px] text-zinc-500">
          <span><span className="text-zinc-300">{skill.deploys.toLocaleString()}</span> deploys</span>
          <span className="size-1 rounded-full bg-zinc-700" />
          <span>v0.{Math.floor(skill.deploys / 100)}</span>
        </div>
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

function Catalog({ skills, onDeploy, deployedSet }) {
  const [q, setQ] = React.useState("");
  const [tier, setTier] = React.useState("All");
  const tiers = ["All", "Mature", "Stable", "Preview", "Emergent"];
  const filtered = skills.filter(s =>
    (tier === "All" || s.tier === tier) &&
    (q.trim() === "" || s.name.toLowerCase().includes(q.toLowerCase()) || s.desc.toLowerCase().includes(q.toLowerCase()))
  );

  const onMove = (e) => {
    const r = e.currentTarget.getBoundingClientRect();
    e.currentTarget.style.setProperty("--mx", `${e.clientX - r.left}px`);
    e.currentTarget.style.setProperty("--my", `${e.clientY - r.top}px`);
  };

  return (
    <div className="flex flex-col gap-5">
      <Glass className="p-5">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Skills</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">First-party plugins · select a capability to dispatch as a task</p>
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
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4" onMouseMove={onMove}>
        {filtered.map(s => <SkillCard key={s.id} skill={s} onDeploy={onDeploy} pending={deployedSet.has(s.id)} />)}
      </div>
    </div>
  );
}

Object.assign(window, { Catalog });
