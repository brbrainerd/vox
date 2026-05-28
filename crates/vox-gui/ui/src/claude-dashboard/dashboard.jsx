// Imperium Dashboard — the Stream + Ludus alerts + active agent rail

function StreamCard({ item, onDoubt, onOverrule }) {
  const tone = {
    validated:   { phase: "Validated",   icon: <Icon.check className="size-3.5" />, bar: "from-emerald-400/40 to-emerald-400/0" },
    "in-progress": { phase: "Executing", icon: <Icon.bolt className="size-3.5" />,  bar: "from-brass/40 to-brass/0" },
    doubted:     { phase: "Doubted",     icon: <Icon.doubt className="size-3.5" />, bar: "from-amber-400/40 to-amber-400/0" },
    speculative: { phase: "Speculative", icon: <Icon.spark className="size-3.5" />, bar: "from-violet-400/40 to-violet-400/0" },
  }[item.kind];

  return (
    <div className="group relative overflow-hidden rounded-xl border border-white/5 bg-white/[0.02] p-3.5 transition hover:border-white/15 hover:bg-white/[0.035] hover:translate-y-[-1px]">
      <div className={`pointer-events-none absolute inset-y-0 left-0 w-[3px] bg-gradient-to-b ${tone.bar}`} />
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 flex-col gap-1.5">
          <div className="flex items-center gap-2">
            <Pill phase={tone.phase} />
            <span className="font-mono text-[10px] text-zinc-500">{item.id}</span>
            <span className="text-[10px] text-zinc-600">·</span>
            <span className="font-display text-[10px] tracking-widest uppercase text-zinc-500">{item.tag}</span>
          </div>
          <div className="font-display text-[14px] font-medium tracking-tight text-zinc-100">{item.title}</div>
          <div className="text-[12px] leading-relaxed text-zinc-400">{item.body}</div>
        </div>
        <div className="flex shrink-0 flex-col items-end gap-2">
          <span className="font-mono text-[10px] text-zinc-500">{item.ts}</span>
          <div className="flex items-center gap-1 opacity-0 transition group-hover:opacity-100">
            {item.kind !== "doubted" && (
              <button onClick={() => onDoubt?.(item)} className="rounded-md border border-white/5 bg-white/[0.02] p-1.5 text-zinc-400 hover:border-amber-400/30 hover:text-amber-300 transition" title="Doubt">
                <Icon.doubt className="size-3.5" />
              </button>
            )}
            {item.kind === "doubted" && (
              <button onClick={() => onOverrule?.(item)} className="rounded-md border border-amber-400/20 bg-amber-400/5 p-1.5 text-amber-300 hover:bg-amber-400/15 transition" title="Overrule">
                <Icon.gavel className="size-3.5" />
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function AgentRow({ a, onPause, onResume }) {
  const t = PHASE_TONE[a.phase];
  const pct = Math.round(a.progress * 100);
  const bp = (a.cost / a.budget) * 100;
  return (
    <div className="group relative rounded-xl border border-white/5 bg-white/[0.015] p-3 transition hover:border-white/10 hover:bg-white/[0.03]">
      <div className="flex items-center gap-3">
        <div className={`relative flex size-9 items-center justify-center rounded-lg bg-white/[0.04] ring-1 ${t.ring} ${t.glow}`}>
          <span className={`font-display text-[11px] font-bold tracking-wider ${t.text}`}>{a.id.replace("A-","")}</span>
        </div>
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex items-center gap-2">
            <span className="font-display text-[12px] font-medium tracking-wide text-zinc-100">{a.codename}</span>
            <Pill phase={a.phase} className="scale-90 origin-left" />
          </div>
          <div className="mt-0.5 truncate text-[11px] text-zinc-500">{a.task}</div>
        </div>
        <div className="flex items-center gap-3">
          <div className="text-right">
            <div className="font-mono text-[11px] tabular-nums text-zinc-200">${a.cost.toFixed(2)}<span className="text-zinc-600"> / ${a.budget.toFixed(2)}</span></div>
            <div className="font-mono text-[10px] text-zinc-500">eta {a.eta}</div>
          </div>
          <button onClick={() => (a.phase === "Paused" ? onResume(a) : onPause(a))} className="rounded-md border border-white/5 bg-white/[0.02] p-1.5 text-zinc-400 hover:border-white/20 hover:text-zinc-100 transition">
            {a.phase === "Paused" ? <Icon.play className="size-3.5" /> : <Icon.pause className="size-3.5" />}
          </button>
        </div>
      </div>
      <div className="mt-2.5 flex items-center gap-2">
        <div className="relative h-1 flex-1 overflow-hidden rounded-full bg-white/5">
          <div className={`absolute inset-y-0 left-0 rounded-full ${a.phase === "Verifying" ? "bg-violet-400" : a.phase === "Executing" ? "bg-brass" : a.phase === "Planning" ? "bg-cyan-400" : "bg-zinc-500"}`} style={{ width: `${pct}%` }} />
          <div className="absolute inset-y-0 left-0 w-full bg-[linear-gradient(90deg,transparent,rgba(255,255,255,0.18),transparent)] animate-vox-shimmer" style={{ width: `${pct}%` }} />
        </div>
        <span className="w-9 text-right font-mono text-[10px] text-zinc-500 tabular-nums">{pct}%</span>
        <div className="ml-1 h-1 w-12 overflow-hidden rounded-full bg-white/5">
          <div className={`h-full ${bp > 80 ? "bg-rose-400" : bp > 50 ? "bg-amber-400" : "bg-emerald-400"}`} style={{ width: `${Math.min(100, bp)}%` }} />
        </div>
      </div>
    </div>
  );
}

function LudusBanner({ note, onAck }) {
  const styling = {
    ok:     { ring: "ring-emerald-400/25", bg: "bg-gradient-to-br from-emerald-500/[0.08] via-emerald-500/[0.02] to-transparent", text: "text-emerald-300", icon: <Icon.check className="size-4"/> },
    warn:   { ring: "ring-amber-400/25", bg: "bg-gradient-to-br from-amber-500/[0.08] via-amber-500/[0.02] to-transparent", text: "text-amber-300", icon: <Icon.alert className="size-4"/> },
    info:   { ring: "ring-cyan-400/25", bg: "bg-gradient-to-br from-cyan-500/[0.08] via-cyan-500/[0.02] to-transparent", text: "text-cyan-300", icon: <Icon.spark className="size-4"/> },
  }[note.level] || { ring: "ring-white/10", bg: "", text: "text-zinc-300", icon: <Icon.alert className="size-4"/> };
  return (
    <div className={`relative overflow-hidden rounded-xl ring-1 ${styling.ring} ${styling.bg} p-3`}>
      <div className="flex items-start gap-3">
        <div className={`flex size-8 shrink-0 items-center justify-center rounded-lg bg-white/[0.04] ring-1 ring-white/5 ${styling.text}`}>{styling.icon}</div>
        <div className="min-w-0 flex-1">
          <div className={`font-display text-[12px] font-medium tracking-wide ${styling.text}`}>{note.title}</div>
          <div className="mt-0.5 text-[11px] leading-relaxed text-zinc-400">{note.body}</div>
        </div>
        <button onClick={() => onAck(note)} className="rounded-md border border-white/5 bg-white/[0.03] p-1 text-zinc-500 hover:text-zinc-200 transition" title="Ack">
          <Icon.x className="size-3.5" />
        </button>
      </div>
    </div>
  );
}

function Dashboard({ data, onPause, onResume, onDoubt, onOverrule, onAckLudus, filterKind, setFilterKind }) {
  const filters = ["all", "validated", "in-progress", "doubted", "speculative"];
  const stream = data.stream.filter(s => filterKind === "all" ? true : s.kind === filterKind);

  return (
    <div className="grid grid-cols-12 gap-5">
      {/* Left: The Stream */}
      <Glass className="col-span-12 xl:col-span-8 p-5">
        <div className="flex items-center justify-between">
          <div>
            <div className="flex items-center gap-3">
              <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">The Stream</h2>
              <span className="rounded-full border border-white/5 bg-white/[0.02] px-2 py-0.5 font-mono text-[10px] text-zinc-500">{stream.length} events</span>
            </div>
            <p className="mt-0.5 text-[11px] text-zinc-500">Mission-control feed · live agent telemetry</p>
          </div>
          <div className="flex gap-1 rounded-lg border border-white/5 bg-white/[0.02] p-1">
            {filters.map(f => (
              <button key={f} onClick={() => setFilterKind(f)} className={`rounded-md px-2.5 py-1 text-[10px] font-display uppercase tracking-wider transition ${filterKind === f ? "bg-white/10 text-zinc-100" : "text-zinc-500 hover:text-zinc-300"}`}>
                {f === "in-progress" ? "in-prog" : f}
              </button>
            ))}
          </div>
        </div>
        <div className="mt-4 flex flex-col gap-2.5">
          {stream.map(s => <StreamCard key={s.id} item={s} onDoubt={onDoubt} onOverrule={onOverrule} />)}
        </div>
      </Glass>

      {/* Right: Ludus + Active Agents */}
      <div className="col-span-12 xl:col-span-4 flex flex-col gap-5">
        <Glass className="p-5">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Icon.alert className="size-4 text-amber-300" />
              <h3 className="font-display text-[14px] font-semibold tracking-wide text-zinc-100">System · Telemetry &amp; Alerts</h3>
            </div>
            <span className="font-mono text-[10px] text-zinc-500">{data.alerts.length} open</span>
          </div>
          <div className="mt-3 flex flex-col gap-2">
            {data.alerts.map(n => <LudusBanner key={n.id} note={n} onAck={onAckLudus} />)}
          </div>
        </Glass>

        <Glass className="p-5">
          <div className="flex items-center justify-between">
            <h3 className="font-display text-[14px] font-semibold tracking-wide text-zinc-100">Active Agents</h3>
            <span className="font-mono text-[10px] text-zinc-500">{data.agents.length} shards</span>
          </div>
          <div className="mt-3 flex flex-col gap-2">
            {data.agents.map(a => <AgentRow key={a.id} a={a} onPause={onPause} onResume={onResume} />)}
          </div>
        </Glass>
      </div>
    </div>
  );
}

Object.assign(window, { Dashboard });
