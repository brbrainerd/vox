// Master App: sidebar + top HUD + main content + Loquela terminal
// Wires voxTransport tool calls from every interaction surface.

const { useState: appUse, useEffect: appEff, useMemo: appMemo, useCallback: appCb } = React;

function NavItem({ active, icon, label, onClick, badge, collapsed }) {
  return (
    <button onClick={onClick} title={collapsed ? label : undefined}
      className={`group relative flex w-full items-center ${collapsed ? "justify-center px-0" : "gap-3 px-3"} py-2.5 rounded-xl transition ${active ? "bg-white/[0.04] text-zinc-100" : "text-zinc-500 hover:bg-white/[0.025] hover:text-zinc-200"}`}>
      {active && <span className="absolute left-0 top-1/2 -translate-y-1/2 h-5 w-[2px] rounded-r bg-brass shadow-[0_0_12px_2px_rgba(212,175,55,0.5)]" />}
      <span className={`flex size-7 items-center justify-center rounded-lg shrink-0 ${active ? "bg-brass/10 text-brass ring-1 ring-brass/30" : "bg-white/[0.02] ring-1 ring-white/5"}`}>{icon}</span>
      {!collapsed && <span className="flex-1 text-left font-display text-[12px] tracking-[0.12em] uppercase whitespace-nowrap overflow-hidden">{label}</span>}
      {!collapsed && badge != null && <span className="rounded-full bg-white/[0.05] px-1.5 py-0.5 font-mono text-[9px] text-zinc-400">{badge}</span>}
      {collapsed && badge != null && <span className="absolute right-1 top-1 rounded-full bg-brass/80 px-1 font-mono text-[8px] text-zinc-950">{badge}</span>}
    </button>
  );
}

const SIDEBAR_WIDTHS = { rail: 64, default: 212, wide: 280 };
const SIDEBAR_ORDER = ["rail", "default", "wide"];

function Sidebar({ view, setView, agentsCount, data, mode, setMode, pushToast }) {
  const w = SIDEBAR_WIDTHS[mode];
  const collapsed = mode === "rail";
  const wide = mode === "wide";
  const cycle = (dir) => {
    const i = SIDEBAR_ORDER.indexOf(mode);
    const ni = Math.max(0, Math.min(SIDEBAR_ORDER.length - 1, i + dir));
    setMode(SIDEBAR_ORDER[ni]);
  };
  return (
    <aside className="shrink-0 flex flex-col transition-[width] duration-200 ease-out" style={{ width: w }}>
      <Glass className="flex h-full flex-col p-3">
        {/* Brand + collapse handles */}
        <div className={`flex items-center ${collapsed ? "justify-center" : "justify-between"} pb-3`}>
          {!collapsed && (
            <div className="flex items-center gap-2 px-1">
              <div className="relative size-6 rounded-md bg-gradient-to-br from-brass via-amber-600 to-zinc-900 ring-1 ring-brass/40">
                <span className="absolute inset-0 grid place-items-center font-display text-[11px] font-bold text-zinc-950">V</span>
              </div>
              <div className="leading-tight">
                <div className="font-display text-[11px] tracking-[0.22em] text-zinc-200">VOX</div>
                <div className="font-mono text-[8px] tracking-widest text-zinc-500">IMPERIUM</div>
              </div>
            </div>
          )}
          <div className={`flex items-center ${collapsed ? "flex-col gap-1" : "gap-0.5"}`}>
            <button onClick={() => cycle(-1)} disabled={mode === "rail"} title="Collapse"
              className={`flex size-6 items-center justify-center rounded-md border border-white/5 ${mode === "rail" ? "opacity-30 cursor-not-allowed" : "hover:bg-white/5 text-zinc-400 hover:text-zinc-100"}`}>
              <Icon.chevL className="size-3"/>
            </button>
            <button onClick={() => cycle(1)} disabled={mode === "wide"} title="Expand"
              className={`flex size-6 items-center justify-center rounded-md border border-white/5 ${mode === "wide" ? "opacity-30 cursor-not-allowed" : "hover:bg-white/5 text-zinc-400 hover:text-zinc-100"}`}>
              <Icon.chevR className="size-3"/>
            </button>
          </div>
        </div>

        {!collapsed && (
          <div className="px-2 pb-3">
            <div className="font-display text-[9px] uppercase tracking-[0.32em] text-zinc-500">Sectors</div>
          </div>
        )}
        <nav className="flex flex-col gap-1">
          <NavItem collapsed={collapsed} active={view === "dashboard"} onClick={() => setView("dashboard")} icon={<Icon.dashboard className="size-4"/>} label="Imperium" />
          <NavItem collapsed={collapsed} active={view === "flow"}      onClick={() => setView("flow")}      icon={<Icon.flow className="size-4"/>}      label="Agents" badge={agentsCount} />
          <NavItem collapsed={collapsed} active={view === "catalog"}   onClick={() => setView("catalog")}   icon={<Icon.catalog className="size-4"/>}   label="Skills" />
          <NavItem collapsed={collapsed} active={view === "matrix"}    onClick={() => setView("matrix")}    icon={<Icon.matrix className="size-4"/>}    label="Policies" />
          <NavItem collapsed={collapsed} active={view === "memory"}    onClick={() => setView("memory")}    icon={<Icon.memory className="size-4"/>}    label="Memory" />
        </nav>

        <div className="mt-auto flex flex-col gap-2 pt-3">
          {!collapsed && (
            <div className="rounded-xl border border-white/5 bg-gradient-to-br from-violet-500/[0.06] via-zinc-900/40 to-zinc-950 p-3">
              <div className="flex items-center gap-2">
                <div className="flex size-7 items-center justify-center rounded-md bg-violet-400/15 text-violet-300"><Icon.cpu className="size-3.5"/></div>
                <div className="leading-tight">
                  <div className="font-display text-[11px] tracking-wider text-violet-300">Mesh</div>
                  <div className="font-mono text-[10px] text-zinc-400">{(data.peers||[]).filter(p=>p.online).length}/{(data.peers||[]).length} peers · 96GB VRAM</div>
                </div>
              </div>
              <div className="mt-2.5 space-y-1">
                {(data.peers||[]).slice(0, wide ? 5 : 3).map(p => (
                  <div key={p.id} className="flex items-center justify-between font-mono text-[9px]">
                    <span className="flex items-center gap-1.5 text-zinc-400"><span className={`size-1 rounded-full ${p.online?"bg-emerald-400":"bg-zinc-600"}`}/>{p.name}</span>
                    <span className="text-zinc-500">{p.backend}</span>
                  </div>
                ))}
              </div>
              {wide && (
                <button onClick={() => pushToast({ tone: "ok", title: "Mesh refreshed", cmd: "mesh_refresh_peers" })}
                  className="mt-3 flex w-full items-center justify-center gap-1.5 rounded-md border border-white/10 bg-white/[0.02] py-1 font-mono text-[10px] text-zinc-300 hover:bg-white/5">
                  <Icon.refresh className="size-3"/> rescan peers
                </button>
              )}
            </div>
          )}
          <NavItem collapsed={collapsed} active={view === "settings"} onClick={() => setView("settings")} icon={<Icon.settings className="size-4"/>} label="Settings" />
          <div className={`flex items-center ${collapsed ? "justify-center" : "gap-2 px-2"} pb-1 pt-1`}>
            <div className="relative size-7 shrink-0 rounded-full bg-gradient-to-br from-violet-500 to-cyan-500">
              <span className="absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full bg-emerald-400 ring-2 ring-zinc-950"/>
            </div>
            {!collapsed && (
              <div className="flex-1 leading-tight overflow-hidden">
                <div className="font-display text-[11px] text-zinc-200 truncate">archon@vox</div>
                <div className="font-mono text-[9px] text-zinc-500">build 0.7.42 · tauri 2</div>
              </div>
            )}
          </div>
        </div>
      </Glass>
    </aside>
  );
}

// — Command palette (⌘K) —————————————————————————————————————————————
function CommandPalette({ open, onClose, onAction, agents, skills }) {
  const [q, setQ] = appUse("");
  appEff(() => {
    if (!open) return;
    const onKey = (e) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);
  if (!open) return null;
  const commands = [
    { id: "submit", label: "Submit new task…", kind: "act", hint: "loquela" },
    { id: "pause-all", label: "Pause all agents", kind: "act" },
    { id: "resume-all", label: "Resume all agents", kind: "act" },
    { id: "ack-all", label: "Acknowledge Ludus alerts", kind: "act" },
    ...agents.map(a => ({ id: `agent:${a.id}`, label: `Agent · ${a.codename} (${a.id})`, kind: "nav" })),
    ...skills.map(s => ({ id: `skill:${s.id}`, label: `Deploy skill · ${s.name}`, kind: "act" })),
  ].filter(c => q === "" || c.label.toLowerCase().includes(q.toLowerCase()));

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/60 backdrop-blur-sm pt-[14vh]" onClick={onClose}>
      <div className="w-[640px] max-w-[92vw] rounded-2xl border border-white/10 bg-zinc-950/90 shadow-[0_40px_120px_-30px_rgba(0,0,0,0.9)] backdrop-blur-2xl" onClick={e => e.stopPropagation()}>
        <div className="flex items-center gap-2 border-b border-white/5 px-4 py-3">
          <Icon.command className="size-4 text-brass"/>
          <input autoFocus value={q} onChange={e=>setQ(e.target.value)} placeholder="Type a command, agent, or skill…" className="flex-1 bg-transparent text-[14px] text-zinc-100 placeholder:text-zinc-600 outline-none"/>
          <kbd className="rounded border border-white/10 bg-white/5 px-1.5 py-0.5 font-mono text-[10px] text-zinc-500">esc</kbd>
        </div>
        <div className="max-h-[380px] overflow-auto p-2">
          {commands.length === 0 && <div className="px-3 py-6 text-center text-[12px] text-zinc-500">No matches.</div>}
          {commands.map(c => (
            <button key={c.id} onClick={() => { onAction(c); onClose(); }} className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-left hover:bg-white/[0.04]">
              <span className="text-[13px] text-zinc-200">{c.label}</span>
              <span className="font-mono text-[9px] uppercase tracking-widest text-zinc-500">{c.kind}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

// — Toast queue ————————————————————————————————————————————————————————
function Toasts({ items, onClose }) {
  return (
    <div className="pointer-events-none fixed bottom-[200px] right-6 z-40 flex w-[320px] flex-col gap-2">
      {items.map(t => (
        <div key={t.id} className="pointer-events-auto rounded-xl border border-white/10 bg-zinc-950/90 p-3 backdrop-blur-xl shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)] animate-vox-toast-in">
          <div className="flex items-start gap-2">
            <div className={`mt-0.5 flex size-6 shrink-0 items-center justify-center rounded ${t.tone === "ok" ? "bg-emerald-400/15 text-emerald-300" : t.tone === "warn" ? "bg-amber-400/15 text-amber-300" : "bg-cyan-400/15 text-cyan-300"}`}>
              {t.tone === "ok" ? <Icon.check className="size-3.5"/> : t.tone === "warn" ? <Icon.alert className="size-3.5"/> : <Icon.bolt className="size-3.5"/>}
            </div>
            <div className="flex-1 leading-tight">
              <div className="font-display text-[12px] tracking-wide text-zinc-100">{t.title}</div>
              {t.body && <div className="mt-0.5 text-[11px] text-zinc-400">{t.body}</div>}
              {t.cmd && <div className="mt-1 font-mono text-[10px] text-zinc-500">▸ {t.cmd}</div>}
            </div>
            <button onClick={() => onClose(t.id)} className="text-zinc-500 hover:text-zinc-100"><Icon.x className="size-3.5"/></button>
          </div>
        </div>
      ))}
    </div>
  );
}

// — Memory + Settings stubs ————————————————————————————————————————————
// ---------- Memory ------------------------------------------------
const MEM_CORPORA = [
  { id: "proj",   name: "Project · vox",         entries: 18492, type: "code",   tone: "text-brass" },
  { id: "docs",   name: "Docs · README + RFCs",   entries: 1240,  type: "text",   tone: "text-cyan-300" },
  { id: "chats",  name: "Chats · Loquela log",     entries: 4310,  type: "chat",   tone: "text-violet-300" },
  { id: "rules",  name: "Rule packs",              entries: 86,    type: "policy", tone: "text-amber-300" },
  { id: "web",    name: "Web crawl · socrates",     entries: 9213,  type: "web",    tone: "text-emerald-300" },
];
const MEM_RECENT = [
  { q: "ed25519 invariant constraints",          n: 12, when: "2m" },
  { q: "qlora epoch schedule for mens-candle",    n: 7,  when: "14m" },
  { q: "durable checkpoint stall mitigation",     n: 21, when: "1h" },
  { q: "vox-arch-check rule precedence",          n: 4,  when: "3h" },
];
const MEM_SAMPLE_HITS = [
  { src: "vox-protocol/src/handshake.rs",   line: 142, score: 0.94, kind: "code",  text: "Constant-time comparison enforced via subtle::ConstantTimeEq; doubt injected when nonce reuse is observed." },
  { src: "docs/rfcs/0007-clavis-rotation.md", line: 18,  score: 0.91, kind: "text",  text: "Rotation cadence is policy-driven (default 90d); break-glass override requires capability gate." },
  { src: "chats/loquela/2026-05-09",         line: 0,   score: 0.88, kind: "chat",  text: "User asked Augur to harden cryptographic invariants — produced 3 candidate branches." },
  { src: "rule-packs/crypto.yml",            line: 31,  score: 0.84, kind: "policy",text: "All AEAD constructions MUST tag nonce-reuse domain separately; doubt threshold 0.7." },
  { src: "vox-arch-check/lib.rs",            line: 88,  score: 0.79, kind: "code",  text: "Rule R-0421: invariants over crypto primitives validated against Socrates citation set." },
];

function MemoryView({ pushToast }) {
  const [query, setQuery]     = appUse("");
  const [scope, setScope]     = appUse(["proj","docs","chats","rules","web"]);
  const [topK, setTopK]       = appUse(8);
  const [recallOn, setRecall] = appUse(false);
  const [hits, setHits]       = appUse([]);
  const [recalling, setRecalling] = appUse(false);

  const totalEntries = MEM_CORPORA.reduce((s, c) => s + (scope.includes(c.id) ? c.entries : 0), 0);
  const toggleScope = (id) => setScope(s => s.includes(id) ? s.filter(x => x !== id) : [...s, id]);

  const recall = (q) => {
    const qq = (q || query).trim();
    if (!qq) return;
    setRecalling(true); setHits([]);
    setTimeout(() => {
      setHits(MEM_SAMPLE_HITS.filter(h => scope.includes({code:"proj",text:"docs",chat:"chats",policy:"rules",web:"web"}[h.kind] || "proj")).slice(0, topK).map(h => ({...h, score: +(h.score - Math.random()*0.04).toFixed(3)})));
      setRecalling(false);
      pushToast && pushToast({ tone: "ok", title: "Recall complete", body: `Top ${topK} matches across ${scope.length} corpora`, cmd: `mnemosyne_recall • "${qq}"` });
    }, 700);
  };

  return (
    <div className="grid grid-cols-12 gap-5">
      <Glass className="col-span-12 p-5">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Mnemosyne · Memory</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">Vector + symbolic recall over {totalEntries.toLocaleString()} entries across {scope.length} corpora</p>
          </div>
          <div className="flex items-center gap-2">
            <button onClick={() => setRecall(r => !r)} className={`inline-flex items-center gap-1.5 rounded-md border px-2 py-1.5 font-mono text-[10px] transition ${recallOn ? "border-violet-400/40 bg-violet-400/10 text-violet-300" : "border-white/10 bg-white/[0.02] text-zinc-400 hover:text-zinc-200"}`}><Icon.eye className="size-3"/> Auto-recall</button>
            <button onClick={() => pushToast({ tone: "warn", title: "Reindex queued", cmd: "mnemosyne_reindex" })} className="inline-flex items-center gap-1.5 rounded-md border border-white/10 bg-white/[0.02] px-2 py-1.5 font-mono text-[10px] text-zinc-400 hover:text-zinc-200"><Icon.refresh className="size-3"/> Reindex</button>
          </div>
        </div>

        {/* Search bar */}
        <div className="mt-4 flex items-center gap-2 rounded-xl border border-white/10 bg-white/[0.02] px-3 py-2">
          <Icon.search className="size-3.5 text-zinc-500"/>
          <input value={query} onChange={e => setQuery(e.target.value)} onKeyDown={e => e.key === "Enter" && recall()} placeholder="Recall… e.g. ‘ed25519 invariants’, ‘checkpoint stall’" className="flex-1 bg-transparent text-[13px] text-zinc-100 placeholder:text-zinc-600 outline-none"/>
          <span className="font-mono text-[10px] text-zinc-500">top</span>
          <input type="number" min={1} max={50} value={topK} onChange={e => setTopK(parseInt(e.target.value || 8))} className="w-12 rounded border border-white/10 bg-white/[0.02] px-1.5 py-0.5 text-center font-mono text-[11px] text-zinc-200 outline-none"/>
          <button onClick={() => recall()} disabled={!query.trim() || recalling} className={`inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1 font-display text-[10px] uppercase tracking-widest transition ${query.trim() ? "border-brass/40 bg-brass/15 text-brass hover:bg-brass/25" : "border-white/5 bg-white/[0.02] text-zinc-600 cursor-not-allowed"}`}>{recalling ? "…" : "Recall"}</button>
        </div>

        {/* Scope chips */}
        <div className="mt-3 flex flex-wrap items-center gap-1.5">
          <span className="font-display text-[9px] uppercase tracking-[0.22em] text-zinc-500">Scope</span>
          {MEM_CORPORA.map(c => {
            const on = scope.includes(c.id);
            return (
              <button key={c.id} onClick={() => toggleScope(c.id)} className={`inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 font-mono text-[10px] transition ${on ? "border-white/15 bg-white/[0.04] text-zinc-100" : "border-white/5 bg-white/[0.01] text-zinc-500"}`}>
                <span className={`size-1.5 rounded-full ${on ? "bg-brass" : "bg-white/15"}`}/>{c.name}
                <span className="text-zinc-500">· {c.entries.toLocaleString()}</span>
              </button>
            );
          })}
        </div>
      </Glass>

      {/* Recent recalls */}
      <Glass className="col-span-12 xl:col-span-4 p-5">
        <div className="flex items-center justify-between">
          <h3 className="font-display text-[13px] uppercase tracking-[0.18em] text-zinc-200">Recent recalls</h3>
          <Icon.clock className="size-3.5 text-zinc-500"/>
        </div>
        <div className="mt-3 space-y-1.5">
          {MEM_RECENT.map((r, i) => (
            <button key={i} onClick={() => { setQuery(r.q); recall(r.q); }} className="flex w-full items-center justify-between rounded-md border border-white/5 bg-white/[0.02] px-2.5 py-1.5 text-left hover:border-white/15 hover:bg-white/[0.04]">
              <div className="min-w-0">
                <div className="truncate text-[12px] text-zinc-200">{r.q}</div>
                <div className="font-mono text-[9px] text-zinc-500">{r.n} hits · {r.when} ago</div>
              </div>
              <Icon.chevR className="size-3 text-zinc-500"/>
            </button>
          ))}
        </div>
      </Glass>

      {/* Hits */}
      <Glass className="col-span-12 xl:col-span-8 p-5">
        <div className="flex items-center justify-between">
          <h3 className="font-display text-[13px] uppercase tracking-[0.18em] text-zinc-200">Citations {hits.length > 0 && <span className="text-zinc-500">· {hits.length}</span>}</h3>
          {hits.length > 0 && <button onClick={() => pushToast({ tone: "ok", title: "Pinned to context", body: `${hits.length} citations attached → Loquela`, cmd: "context.attach" })} className="inline-flex items-center gap-1 rounded-md border border-cyan-400/30 bg-cyan-400/10 px-2 py-1 font-mono text-[10px] text-cyan-300 hover:bg-cyan-400/15"><Icon.pin className="size-3"/> Pin all to context</button>}
        </div>
        <div className="mt-3 space-y-2">
          {recalling && Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="h-12 rounded-md border border-white/5 bg-white/[0.02] relative overflow-hidden"><span className="absolute inset-0 -translate-x-full animate-vox-shimmer bg-gradient-to-r from-transparent via-white/5 to-transparent"/></div>
          ))}
          {!recalling && hits.length === 0 && (
            <div className="flex flex-col items-center justify-center rounded-xl border border-dashed border-white/10 bg-white/[0.01] py-12 text-center">
              <Icon.memory className="size-6 text-zinc-600 mb-2"/>
              <div className="font-display text-[12px] tracking-wider text-zinc-400">No recall yet</div>
              <div className="font-mono text-[10px] text-zinc-500">Type a query or click a recent recall to surface citations</div>
            </div>
          )}
          {!recalling && hits.map((h, i) => (
            <div key={i} className="group flex items-start gap-3 rounded-md border border-white/5 bg-white/[0.02] p-3 hover:border-white/15">
              <div className="flex size-7 shrink-0 items-center justify-center rounded bg-white/[0.03] text-zinc-400">
                {h.kind === "code" ? <Icon.file className="size-3.5"/> : h.kind === "text" ? <Icon.catalog className="size-3.5"/> : h.kind === "chat" ? <Icon.bolt className="size-3.5"/> : h.kind === "policy" ? <Icon.shield className="size-3.5"/> : <Icon.link className="size-3.5"/>}
              </div>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="font-mono text-[11px] text-zinc-300 truncate">{h.src}</span>
                  {h.line > 0 && <span className="font-mono text-[9px] text-zinc-500">:{h.line}</span>}
                </div>
                <div className="mt-1 text-[12px] leading-relaxed text-zinc-300 line-clamp-2">{h.text}</div>
              </div>
              <div className="flex flex-col items-end gap-1">
                <span className="font-mono text-[10px] tabular-nums text-emerald-300">{(h.score*100).toFixed(1)}%</span>
                <div className="h-1 w-16 overflow-hidden rounded-full bg-white/5"><div className="h-full bg-gradient-to-r from-violet-400 to-emerald-400" style={{ width: `${h.score*100}%` }}/></div>
                <button onClick={() => pushToast({ tone: "ok", title: "Cited", body: h.src, cmd: "context.pin" })} className="opacity-0 group-hover:opacity-100 transition rounded border border-white/10 bg-white/[0.02] px-1.5 py-0.5 font-mono text-[9px] text-zinc-300 hover:bg-white/5"><Icon.pin className="size-2.5 inline"/> pin</button>
              </div>
            </div>
          ))}
        </div>
      </Glass>

      {/* Shards */}
      <Glass className="col-span-12 p-5">
        <div className="flex items-center justify-between">
          <h3 className="font-display text-[13px] uppercase tracking-[0.18em] text-zinc-200">Memory shards</h3>
          <span className="font-mono text-[10px] text-zinc-500">12 live · HNSW · dim 1024</span>
        </div>
        <div className="mt-3 grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-6">
          {Array.from({length: 12}, (_, i) => ({ id: `0x${(0x3A + i).toString(16).toUpperCase()}`, depth: 3 + (i % 4), entries: 1280 + i*173, hot: i < 3, dirty: i === 4 || i === 9 })).map(s => (
            <div key={s.id} className={`rounded-xl border p-3 transition hover:border-white/15 ${s.hot ? "border-brass/30 bg-brass/[0.04]" : s.dirty ? "border-amber-400/30 bg-amber-400/[0.04]" : "border-white/5 bg-white/[0.02]"}`}>
              <div className="flex items-center justify-between">
                <span className="font-mono text-[11px] text-zinc-300">shard-{s.id}</span>
                {s.hot && <span className="rounded-full bg-brass/15 px-1.5 py-0.5 font-display text-[9px] uppercase tracking-widest text-brass">hot</span>}
                {s.dirty && <span className="rounded-full bg-amber-400/15 px-1.5 py-0.5 font-display text-[9px] uppercase tracking-widest text-amber-300">dirty</span>}
              </div>
              <div className="mt-2 grid grid-cols-2 gap-1.5">
                <Stat2 label="Depth" value={s.depth}/>
                <Stat2 label="Entries" value={s.entries.toLocaleString()}/>
              </div>
              <div className="mt-2 h-8"><Sparkline data={[5,6,5,7,8,7,9,10,9,11,12,11,13]} color={s.hot ? "#d4af37" : s.dirty ? "#fbbf24" : "#71717a"} width={160} height={28}/></div>
            </div>
          ))}
        </div>
      </Glass>
    </div>
  );
}
function Stat2({ label, value }) { return <div className="rounded border border-white/5 bg-zinc-950/40 px-2 py-1.5"><div className="text-[9px] uppercase tracking-widest text-zinc-500">{label}</div><div className="mt-0.5 font-mono text-[11px] text-zinc-200">{value}</div></div>; }

function SettingsView({ pushToast }) {
  const [vals, setVals] = appUse({ doubt: true, autobudget: true, theme: "arcane", concurrency: 7, capUsd: 5, doubtThresh: 0.6, sign: false, telemetry: "local", isolation: "wasm", checkpointMins: 5 });
  const [section, setSection] = appUse("orchestrator");
  const SECTIONS = [
    { id: "orchestrator", icon: "cpu",      label: "Orchestrator" },
    { id: "mesh",         icon: "flow",     label: "Mesh & peers" },
    { id: "signing",      icon: "shield",   label: "Signing keys" },
    { id: "telemetry",    icon: "scale",    label: "Telemetry" },
    { id: "keybinds",     icon: "command",  label: "Keybinds" },
    { id: "theme",        icon: "spark",    label: "Theme" },
  ];
  const KEYBINDS = [
    ["⌘K",     "Open command palette"],
    ["⌘↵",   "Dispatch intent"],
    ["⇧↵",   "Newline in composer"],
    ["/",       "Slash command"],
    ["@",       "Mention agent"],
    ["↑/↓",   "History recall"],
    ["⌘B",     "Toggle sidebar"],
    ["⌘.",     "Pause/resume selected agent"],
  ];
  return (
    <div className="grid grid-cols-12 gap-5">
      <Glass className="col-span-12 md:col-span-3 p-3">
        <nav className="flex flex-col gap-1">
          {SECTIONS.map(s => {
            const IcoCmp = Icon[s.icon] || Icon.bolt;
            const on = section === s.id;
            return (
              <button key={s.id} onClick={() => setSection(s.id)} className={`flex items-center gap-2.5 rounded-lg px-3 py-2 text-left transition ${on ? "bg-white/[0.05] text-zinc-100" : "text-zinc-400 hover:bg-white/[0.025] hover:text-zinc-200"}`}>
                <IcoCmp className={`size-4 ${on ? "text-brass" : "text-zinc-500"}`}/>
                <span className="font-display text-[12px] tracking-[0.12em] uppercase">{s.label}</span>
              </button>
            );
          })}
        </nav>
      </Glass>

      <Glass className="col-span-12 md:col-span-9 p-5">
        {section === "orchestrator" && (<>
          <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Orchestrator</h2>
          <p className="mt-0.5 text-[11px] text-zinc-500">Global scheduling, budget, and verification policy</p>
          <div className="mt-4 space-y-3">
            <Row label="Max concurrent agents" hint="Hard cap before scheduler back-pressure"><RangeInline value={vals.concurrency} min={1} max={16} onChange={v => setVals({...vals, concurrency: v})}/></Row>
            <Row label="Global budget cap (USD)" hint="Soft + hard cap. Throttles when reached."><RangeInline value={vals.capUsd} min={1} max={50} step={1} suffix="$" onChange={v => setVals({...vals, capUsd: v})}/></Row>
            <Row label="Auto-doubt threshold" hint="Confidence floor below which Augur intervenes"><RangeInline value={vals.doubtThresh*100} min={0} max={100} step={5} suffix="%" onChange={v => setVals({...vals, doubtThresh: v/100})}/></Row>
            <Row label="Durable checkpoint cadence" hint="Snapshot interval for resumable runs"><RangeInline value={vals.checkpointMins} min={1} max={30} step={1} suffix="min" onChange={v => setVals({...vals, checkpointMins: v})}/></Row>
            <Row label="Default isolation tier" hint="Runtime sandbox for new agents">
              <div className="inline-flex items-center rounded-md border border-white/10 bg-black/30 p-0.5">
                {[["wasm","WASM"],["ctr","Container"],["native","Native"]].map(([id,l]) => (
                  <button key={id} onClick={() => setVals({...vals, isolation: id})} className={`rounded-[5px] px-2 py-1 font-display text-[10px] uppercase tracking-[0.15em] ${vals.isolation===id ? "bg-white/10 text-zinc-50" : "text-zinc-500 hover:text-zinc-300"}`}>{l}</button>
                ))}
              </div>
            </Row>
            <Row label="Auto-doubt unverified outputs" hint="Inject Augur into the verify lane"><Toggle on={vals.doubt} onClick={() => setVals({...vals, doubt: !vals.doubt})}/></Row>
            <Row label="Auto-budget per agent" hint="Derived from skill + recent burn"><Toggle on={vals.autobudget} onClick={() => setVals({...vals, autobudget: !vals.autobudget})}/></Row>
          </div>
        </>)}

        {section === "mesh" && (<>
          <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Mesh & peers</h2>
          <p className="mt-0.5 text-[11px] text-zinc-500">Discover and authorise peer compute on the local mesh</p>
          <div className="mt-4 space-y-2">
            {[
              { name: "forge.local",   backend: "candle-cuda",   vram: "24GB", online: true,  trust: "verified" },
              { name: "oracle.local",  backend: "mlx-metal",     vram: "36GB", online: true,  trust: "verified" },
              { name: "node-03",       backend: "vllm-cuda",     vram: "40GB", online: true,  trust: "pending" },
              { name: "node-04",       backend: "candle-cpu",    vram: "—",     online: false, trust: "revoked" },
            ].map(p => (
              <div key={p.name} className="flex items-center justify-between rounded-md border border-white/5 bg-white/[0.02] p-3">
                <div className="flex items-center gap-3">
                  <span className={`size-2 rounded-full ${p.online ? "bg-emerald-400" : "bg-zinc-600"}`}/>
                  <div className="leading-tight">
                    <div className="font-mono text-[12px] text-zinc-100">{p.name}</div>
                    <div className="font-mono text-[10px] text-zinc-500">{p.backend} · {p.vram}</div>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <span className={`rounded-full px-2 py-0.5 font-display text-[9px] uppercase tracking-widest ${p.trust==="verified" ? "bg-emerald-400/15 text-emerald-300" : p.trust==="pending" ? "bg-amber-400/15 text-amber-300" : "bg-zinc-700/40 text-zinc-400"}`}>{p.trust}</span>
                  <button onClick={() => pushToast({ tone: "ok", title: `${p.name} action`, cmd: "mesh.toggle_trust" })} className="rounded border border-white/10 bg-white/[0.02] px-2 py-1 font-mono text-[10px] text-zinc-300 hover:bg-white/5">manage</button>
                </div>
              </div>
            ))}
          </div>
        </>)}

        {section === "signing" && (<>
          <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Signing keys</h2>
          <p className="mt-0.5 text-[11px] text-zinc-500">ed25519 capability gates for high-risk dispatch</p>
          <div className="mt-4 space-y-2">
            {[
              { id: "clavis-primary",  fp: "ed25519:7F:42:9B…2A:11", rotated: "4d ago", scope: "all" },
              { id: "clavis-readonly", fp: "ed25519:11:CD:8E…77:0A", rotated: "22d ago", scope: "recall-only" },
            ].map(k => (
              <div key={k.id} className="flex items-center justify-between rounded-md border border-white/5 bg-white/[0.02] p-3">
                <div className="flex items-center gap-3">
                  <Icon.shield className="size-4 text-amber-300"/>
                  <div className="leading-tight">
                    <div className="font-mono text-[12px] text-zinc-100">{k.id}</div>
                    <div className="font-mono text-[10px] text-zinc-500">{k.fp} · rotated {k.rotated}</div>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <span className="rounded-full bg-white/[0.04] px-2 py-0.5 font-display text-[9px] uppercase tracking-widest text-zinc-300">{k.scope}</span>
                  <button onClick={() => pushToast({ tone: "warn", title: "Rotation queued", body: k.id, cmd: "clavis.rotate" })} className="rounded border border-white/10 bg-white/[0.02] px-2 py-1 font-mono text-[10px] text-zinc-300 hover:bg-white/5">rotate</button>
                </div>
              </div>
            ))}
            <Row label="Require signature on Native isolation" hint="Hard gate; refuses dispatch without ed25519"><Toggle on={vals.sign} onClick={() => setVals({...vals, sign: !vals.sign})}/></Row>
          </div>
        </>)}

        {section === "telemetry" && (<>
          <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Telemetry</h2>
          <p className="mt-0.5 text-[11px] text-zinc-500">Where Vox sends spans, metrics, and traces</p>
          <div className="mt-4 grid grid-cols-3 gap-2">
            {[["off","Off","Nothing leaves the device"],["local","Local","OTLP → localhost:4317"],["cloud","Cloud","Encrypted → vendor"]].map(([id,l,h]) => (
              <button key={id} onClick={() => setVals({...vals, telemetry: id})} className={`rounded-xl border p-3 text-left transition ${vals.telemetry===id ? "border-brass/40 bg-brass/[0.05]" : "border-white/5 hover:border-white/15 bg-white/[0.02]"}`}>
                <div className="font-display text-[12px] tracking-wider text-zinc-100">{l}</div>
                <div className="mt-1 font-mono text-[10px] text-zinc-500">{h}</div>
              </button>
            ))}
          </div>
        </>)}

        {section === "keybinds" && (<>
          <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Keybinds</h2>
          <p className="mt-0.5 text-[11px] text-zinc-500">Global shortcuts — click to rebind (mock)</p>
          <div className="mt-4 grid grid-cols-1 gap-1.5 md:grid-cols-2">
            {KEYBINDS.map(([k, d]) => (
              <div key={k} className="flex items-center justify-between rounded-md border border-white/5 bg-white/[0.02] px-3 py-2">
                <span className="text-[12px] text-zinc-200">{d}</span>
                <kbd className="rounded border border-white/10 bg-white/5 px-2 py-0.5 font-mono text-[10px] text-zinc-300">{k}</kbd>
              </div>
            ))}
          </div>
        </>)}

        {section === "theme" && (<>
          <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Theme</h2>
          <p className="mt-0.5 text-[11px] text-zinc-500">Aesthetic mode for the GUI layer</p>
          <div className="mt-4 grid grid-cols-2 gap-3 md:grid-cols-3">
            {[
              { id: "arcane", name: "Arcane",   swatch: "from-brass via-amber-600 to-zinc-900" },
              { id: "void",   name: "Void",     swatch: "from-violet-500 via-zinc-800 to-zinc-950" },
              { id: "glacier",name: "Glacier",  swatch: "from-cyan-400 via-slate-700 to-zinc-950" },
            ].map(t => (
              <button key={t.id} onClick={() => setVals({...vals, theme: t.id})} className={`rounded-xl border p-3 text-left transition ${vals.theme === t.id ? "border-brass/40 bg-brass/[0.05]" : "border-white/5 hover:border-white/15 bg-white/[0.02]"}`}>
                <div className={`h-16 w-full rounded-lg bg-gradient-to-br ${t.swatch}`} />
                <div className="mt-2 font-display text-[12px] tracking-wider text-zinc-200">{t.name}</div>
              </button>
            ))}
          </div>
        </>)}
      </Glass>
    </div>
  );
}
function Row({ label, hint, children }) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-xl border border-white/5 bg-white/[0.02] p-3">
      <div><div className="font-display text-[12px] text-zinc-200">{label}</div><div className="text-[11px] text-zinc-500">{hint}</div></div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}
function Toggle({ on, onClick }) { return <button onClick={onClick} className={`relative h-5 w-9 rounded-full transition ${on ? "bg-brass/40" : "bg-white/10"}`}><span className={`absolute top-0.5 size-4 rounded-full bg-zinc-50 transition ${on ? "left-[18px]" : "left-0.5"}`} /></button>; }
function RangeInline({ value, min, max, step = 1, suffix = "", onChange }) {
  return (
    <div className="flex w-56 items-center gap-3">
      <input type="range" min={min} max={max} step={step} value={value} onChange={e => onChange(Number(e.target.value))} className="vox-range flex-1"/>
      <span className="w-12 text-right font-mono text-[11px] text-zinc-200">{suffix}{value}</span>
    </div>
  );
}

// — App root ————————————————————————————————————————————————————————
function App() {
  const fixtures = window.VOX_FIXTURES;
  const [view, setView] = appUse("dashboard");
  const [data, setData] = appUse(fixtures);
  const [filterKind, setFilterKind] = appUse("all");
  const [chips, setChips] = appUse(fixtures.contextChips);
  const [activeSkill, setActiveSkill] = appUse(fixtures.skills[0]);
  const [deployed, setDeployed] = appUse(new Set());
  const [paletteOpen, setPaletteOpen] = appUse(false);
  const [toasts, setToasts] = appUse([]);
  const [selectedNode, setSelectedNode] = appUse("A-01");
  const [sidebarMode, setSidebarMode] = appUse(() => {
    try { return localStorage.getItem("vox.sidebar") || "default"; } catch (e) { return "default"; }
  });
  appEff(() => { try { localStorage.setItem("vox.sidebar", sidebarMode); } catch (e) {} }, [sidebarMode]);

  const pushToast = (t) => {
    const id = "t" + Date.now() + Math.random().toString(36).slice(2,6);
    setToasts(prev => [...prev, { ...t, id }]);
    setTimeout(() => setToasts(prev => prev.filter(x => x.id !== id)), 4200);
  };

  // — voxTransport wrappers
  const submitTask = appCb(async (args) => {
    await voxTransport.callTool("vox_submit_task", args);
    pushToast({ tone: "ok", title: "Intent dispatched", body: args.description.slice(0, 80) + (args.description.length>80?"…":""), cmd: `vox_submit_task • skill: ${args.active_skill || "auto"}` });
  }, []);
  const pauseAgent = appCb(async (a) => {
    await voxTransport.callTool("vox_pause_agent", { agent_id: a.id });
    setData(d => ({ ...d, agents: d.agents.map(x => x.id === a.id ? { ...x, phase: "Paused" } : x) }));
    pushToast({ tone: "warn", title: `${a.codename} paused`, cmd: `vox_pause_agent • ${a.id}` });
  }, []);
  const resumeAgent = appCb(async (a) => {
    await voxTransport.callTool("vox_resume_agent", { agent_id: a.id });
    setData(d => ({ ...d, agents: d.agents.map(x => x.id === a.id ? { ...x, phase: "Executing" } : x) }));
    pushToast({ tone: "ok", title: `${a.codename} resumed`, cmd: `vox_resume_agent • ${a.id}` });
  }, []);
  const doubtTask = appCb(async (item) => {
    await voxTransport.callTool("vox_doubt_task", { task_id: item.id });
    setData(d => ({ ...d, stream: d.stream.map(x => x.id === item.id ? { ...x, kind: "doubted" } : x) }));
    pushToast({ tone: "warn", title: "Task doubted", body: item.title, cmd: `vox_doubt_task • ${item.id}` });
  }, []);
  const overruleTask = appCb(async (item) => {
    await voxTransport.callTool("vox_overrule_task", { task_id: item.id });
    setData(d => ({ ...d, stream: d.stream.map(x => x.id === item.id ? { ...x, kind: "validated" } : x) }));
    pushToast({ tone: "ok", title: "Doubt overruled", body: item.title, cmd: `vox_overrule_task • ${item.id}` });
  }, []);
  const ackLudus = appCb(async (n) => {
    await voxTransport.callTool("vox_gamify_notification_ack", { notification_id: n.id });
    setData(d => ({ ...d, alerts: d.alerts.filter(x => x.id !== n.id) }));
  }, []);
  const deploySkill = appCb(async (s) => {
    await voxTransport.callTool("vox_submit_task", { description: `Deploy skill: ${s.name}`, active_skill: s.id });
    setDeployed(prev => new Set([...prev, s.id]));
    pushToast({ tone: "ok", title: `Deployed ${s.name}`, body: s.desc, cmd: `vox_submit_task • skill: ${s.id}` });
    setTimeout(() => setDeployed(prev => { const n = new Set(prev); n.delete(s.id); return n; }), 2400);
  }, []);

  // Poll orchestrator status (mock)
  appEff(() => {
    const tick = async () => {
      await voxTransport.invoke("get_orchestrator_status");
      setData(d => ({
        ...d,
        kpis: {
          ...d.kpis,
          budgetBurn: { ...d.kpis.budgetBurn, value: Math.min(d.kpis.budgetBurn.cap, +(d.kpis.budgetBurn.value + (Math.random()*0.04 - 0.01)).toFixed(2)), spark: [...d.kpis.budgetBurn.spark.slice(1), d.kpis.budgetBurn.value] },
          mesh: { ...d.kpis.mesh, value: Math.max(0, d.kpis.mesh.value + Math.round(Math.random()*30 - 12)), spark: [...d.kpis.mesh.spark.slice(1), d.kpis.mesh.value] },
        },
        agents: d.agents.map(a => a.phase === "Paused" ? a : ({ ...a, progress: Math.min(1, a.progress + Math.random()*0.012) })),
      }));
    };
    const id = setInterval(tick, 2400);
    return () => clearInterval(id);
  }, []);

  // Global keybinds
  appEff(() => {
    const onKey = (e) => {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === "k") { e.preventDefault(); setPaletteOpen(true); }
      if (mod && e.key.toLowerCase() === "b") {
        e.preventDefault();
        setSidebarMode(m => m === "rail" ? "default" : m === "default" ? "wide" : "rail");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const closeToast = (id) => setToasts(prev => prev.filter(t => t.id !== id));
  const handleCommand = (cmd) => {
    if (cmd.id === "submit") document.querySelector("textarea")?.focus();
    else if (cmd.id === "pause-all") data.agents.forEach(pauseAgent);
    else if (cmd.id === "resume-all") data.agents.filter(a => a.phase === "Paused").forEach(resumeAgent);
    else if (cmd.id === "ack-all") data.alerts.forEach(ackLudus);
    else if (cmd.id.startsWith("agent:")) { setView("flow"); setSelectedNode(cmd.id.slice(6)); }
    else if (cmd.id.startsWith("skill:")) { const s = data.skills.find(x => x.id === cmd.id.slice(6)); if (s) deploySkill(s); }
  };

  return (
    <div className="relative min-h-screen px-5 pt-5">
      <ArcaneBackdrop/>
      <div className="flex gap-5">
        <Sidebar view={view} setView={setView} agentsCount={data.agents.length} data={data} mode={sidebarMode} setMode={setSidebarMode} pushToast={pushToast} />
        <main className="flex min-w-0 flex-1 flex-col gap-5 pb-[220px]">
          <TopHud kpis={data.kpis} onCommand={() => setPaletteOpen(true)} />
          {view === "dashboard" && (
            <Dashboard data={data} onPause={pauseAgent} onResume={resumeAgent} onDoubt={doubtTask} onOverrule={overruleTask} onAckLudus={ackLudus} filterKind={filterKind} setFilterKind={setFilterKind} />
          )}
          {view === "flow" && <AgentFlow graph={data.graph} onSelect={setSelectedNode} selectedId={selectedNode} />}
          {view === "catalog" && <Catalog skills={data.skills} onDeploy={deploySkill} deployedSet={deployed} />}
          {view === "matrix" && <IntentionMatrix intentions={data.policies} onDoubt={doubtTask} onOverrule={overruleTask} />}
          {view === "memory" && <MemoryView pushToast={pushToast} />}
          {view === "settings" && <SettingsView pushToast={pushToast} />}
        </main>
      </div>

      {/* Loquela terminal pinned to bottom — left-padding tracks sidebar width */}
      <div className="fixed inset-x-5 bottom-5 z-30 transition-[padding] duration-200" style={{ paddingLeft: SIDEBAR_WIDTHS[sidebarMode] + 20 }}>
        <Loquela chips={chips} setChips={setChips} onSubmit={submitTask} activeSkill={activeSkill} setActiveSkill={setActiveSkill} skills={data.skills} toast={pushToast} />
      </div>

      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} onAction={handleCommand} agents={data.agents} skills={data.skills} />
      <Toasts items={toasts} onClose={closeToast} />
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App/>);
