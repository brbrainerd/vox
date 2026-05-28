// Loquela Terminal — premium prompt input with full orchestrator controls
const { useState: useState_lq, useRef: useRef_lq, useEffect: useEffect_lq, useMemo: useMemo_lq } = React;

const LQ_MODES = [
  { id: "plan",   label: "Plan",   hint: "Augur drafts a plan, no side effects",       tone: "text-cyan-300   border-cyan-400/30   bg-cyan-400/[0.08]" },
  { id: "act",    label: "Act",    hint: "Plan → execute under risk gates",            tone: "text-brass      border-brass/30      bg-brass/[0.08]" },
  { id: "verify", label: "Verify", hint: "Re-run with stricter doubt + property tests", tone: "text-violet-300 border-violet-400/30 bg-violet-400/[0.08]" },
];

const LQ_TIERS = [
  { id: "local",  label: "Local · Mens",    detail: "candle-cuda · 8B",   cost: 0.00, lat: "0.3s" },
  { id: "mesh",   label: "Mesh · Peers",    detail: "3 peers · 14B",      cost: 0.04, lat: "0.6s" },
  { id: "cloud",  label: "Cloud · Cascade", detail: "Sonnet → Opus",      cost: 0.42, lat: "1.4s" },
  { id: "auto",   label: "Auto · Cascade",  detail: "tier-router decides", cost: null, lat: "var" },
];

const LQ_ISO = [
  { id: "wasm",   label: "WASM",      tone: "text-emerald-300" },
  { id: "ctr",    label: "Container", tone: "text-cyan-300" },
  { id: "native", label: "Native",    tone: "text-amber-300" },
];

const LQ_SLASH = [
  { cmd: "/plan",    desc: "Draft a multi-step plan without executing",    icon: "flow" },
  { cmd: "/spawn",   desc: "Spin up a sub-agent on this branch",           icon: "agent" },
  { cmd: "/audit",   desc: "Socrates citation + invariant audit on file",  icon: "shield" },
  { cmd: "/verify",  desc: "Run rule-pack + property tests",               icon: "check" },
  { cmd: "/doubt",   desc: "Inject doubt at threshold N",                   icon: "alert" },
  { cmd: "/memory",  desc: "Query Mnemosyne (RAG over project memory)",    icon: "memory" },
  { cmd: "/rollback",desc: "Revert to last durable checkpoint",            icon: "back" },
  { cmd: "/diff",    desc: "Show pending diff staged by agent",             icon: "file" },
];

const LQ_AT_AGENTS = [
  { id: "A-01", name: "Oracle",       phase: "Verifying" },
  { id: "A-02", name: "Forge",        phase: "Executing" },
  { id: "A-03", name: "Augur",        phase: "Planning" },
  { id: "A-04", name: "Socrates",     phase: "Auditing" },
  { id: "A-05", name: "Mnemosyne",    phase: "Idle" },
];

function Chip({ chip, onRemove }) {
  const iconKey = { file: "file", skill: "bolt", agent: "cpu", branch: "git", url: "link", image: "image" }[chip.kind] || "file";
  const IconCmp = Icon[iconKey] || Icon.file;
  const tone = chip.kind === "file"   ? "border-cyan-400/25 text-cyan-300 bg-cyan-400/[0.05]"
            : chip.kind === "skill"  ? "border-brass/30 text-brass bg-brass/[0.05]"
            : chip.kind === "agent"  ? "border-violet-400/25 text-violet-300 bg-violet-400/[0.05]"
            : chip.kind === "branch" ? "border-emerald-400/25 text-emerald-300 bg-emerald-400/[0.05]"
            :                          "border-white/10 text-zinc-300 bg-white/[0.03]";
  return (
    <span className={`group inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 font-mono text-[10px] ${tone}`}>
      <IconCmp className="size-3" />
      <span className="truncate max-w-[180px]">{chip.label}</span>
      {chip.meta && <span className="text-zinc-500">· {chip.meta}</span>}
      <button onClick={() => onRemove(chip)} className="ml-0.5 opacity-40 hover:opacity-100"><Icon.x className="size-2.5" /></button>
    </span>
  );
}

function Segment({ value, onChange, options, size = "sm" }) {
  const pad = size === "xs" ? "px-2 py-0.5 text-[10px]" : "px-2.5 py-1 text-[11px]";
  return (
    <div className="inline-flex items-center rounded-md border border-white/10 bg-black/30 p-0.5">
      {options.map(o => {
        const on = value === o.id;
        return (
          <button key={o.id} title={o.hint} onClick={() => onChange(o.id)}
            className={`${pad} font-display uppercase tracking-[0.15em] rounded-[5px] transition ${on ? (o.tone || "bg-white/10 text-zinc-50") : "text-zinc-500 hover:text-zinc-300"}`}>
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

function MiniSlider({ label, value, setValue, min, max, step, fmt, accent = "#d4af37" }) {
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <label className="group flex items-center gap-2">
      <span className="font-display text-[9px] uppercase tracking-[0.22em] text-zinc-500">{label}</span>
      <input type="range" min={min} max={max} step={step} value={value}
        onChange={e => setValue(parseFloat(e.target.value))}
        className="vox-range w-24" style={{ ['--p']: pct + "%", background: `linear-gradient(to right, ${accent} ${pct}%, rgba(255,255,255,0.08) ${pct}%)` }} />
      <span className="w-10 font-mono text-[10px] tabular-nums text-zinc-300">{fmt(value)}</span>
    </label>
  );
}

function Popover({ open, children, align = "left" }) {
  if (!open) return null;
  return (
    <div className={`absolute ${align === "right" ? "right-0" : "left-0"} bottom-9 z-50 min-w-[240px] rounded-lg border border-white/10 bg-zinc-950/95 p-1 backdrop-blur-xl shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)]`}>
      {children}
    </div>
  );
}

function Loquela({ chips, setChips, onSubmit, activeSkill, setActiveSkill, skills, toast }) {
  const [text, setText] = useState_lq("");
  const [mode, setMode] = useState_lq("act");
  const [tier, setTier] = useState_lq("auto");
  const [iso, setIso]   = useState_lq("wasm");
  const [budget, setBudget] = useState_lq(5.0);
  const [doubt,  setDoubt]  = useState_lq(0.6);
  const [stream, setStream] = useState_lq(true);
  const [sign,   setSign]   = useState_lq(false);
  const [dryRun, setDryRun] = useState_lq(false);
  const [mic,    setMic]    = useState_lq(false);

  const [skillOpen, setSkillOpen] = useState_lq(false);
  const [tierOpen,  setTierOpen]  = useState_lq(false);
  const [moreOpen,  setMoreOpen]  = useState_lq(false);
  const [slashOpen, setSlashOpen] = useState_lq(false);
  const [atOpen,    setAtOpen]    = useState_lq(false);
  const [slashIdx,  setSlashIdx]  = useState_lq(0);
  const [focused,   setFocused]   = useState_lq(false);
  const [history,   setHistory]   = useState_lq([]);
  const [histIdx,   setHistIdx]   = useState_lq(-1);
  const [expanded,  setExpanded]  = useState_lq(false);

  const taRef = useRef_lq(null);
  const dropRef = useRef_lq(null);
  const [dragOver, setDragOver] = useState_lq(false);

  // Autosize
  useEffect_lq(() => {
    const ta = taRef.current; if (!ta) return;
    ta.style.height = "auto";
    ta.style.height = Math.min(expanded ? 360 : 200, ta.scrollHeight) + "px";
  }, [text, expanded]);

  // Slash + @ trigger detection
  useEffect_lq(() => {
    const trimmed = text.trimStart();
    setSlashOpen(trimmed.startsWith("/") && !trimmed.includes(" "));
    const m = text.match(/@(\w*)$/);
    setAtOpen(!!m);
    setSlashIdx(0);
  }, [text]);

  const filteredSlash = useMemo_lq(() => {
    const q = text.trimStart().toLowerCase();
    return LQ_SLASH.filter(s => s.cmd.startsWith(q));
  }, [text]);
  const filteredAt = useMemo_lq(() => {
    const m = text.match(/@(\w*)$/); const q = (m?.[1] || "").toLowerCase();
    return LQ_AT_AGENTS.filter(a => a.id.toLowerCase().includes(q) || a.name.toLowerCase().includes(q));
  }, [text]);

  // Token + cost estimate
  const tokens = Math.ceil(text.length / 4) + chips.length * 80;
  const tierObj = LQ_TIERS.find(t => t.id === tier);
  const estCost = tierObj?.cost == null ? null : (tokens / 1000) * tierObj.cost + 0.002;

  const insertSlash = (cmd) => { setText(cmd + " "); setSlashOpen(false); taRef.current?.focus(); };
  const insertAt = (agent) => {
    setText(t => t.replace(/@\w*$/, `@${agent.id} `));
    setChips(cs => cs.find(c => c.id === "agent-" + agent.id) ? cs : [...cs, { id: "agent-" + agent.id, kind: "agent", label: `${agent.id} · ${agent.name}`, meta: agent.phase }]);
    setAtOpen(false); taRef.current?.focus();
  };

  const send = () => {
    if (!text.trim()) return;
    const payload = {
      description: text.trim(),
      active_skill: activeSkill?.id,
      mode, tier, isolation: iso,
      max_cost_usd: budget,
      doubt_threshold: doubt,
      stream, require_signature: sign, dry_run: dryRun,
      context: chips.map(c => ({ kind: c.kind, ref: c.label })),
    };
    onSubmit(payload);
    setHistory(h => [text.trim(), ...h].slice(0, 30));
    setHistIdx(-1);
    setText("");
  };

  const onKey = (e) => {
    if (slashOpen && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      e.preventDefault();
      setSlashIdx(i => (i + (e.key === "ArrowDown" ? 1 : -1) + filteredSlash.length) % Math.max(1, filteredSlash.length));
      return;
    }
    if (slashOpen && e.key === "Enter") { e.preventDefault(); const s = filteredSlash[slashIdx]; if (s) insertSlash(s.cmd); return; }
    if (slashOpen && e.key === "Escape") { setSlashOpen(false); return; }
    if (e.key === "ArrowUp" && !text && history.length) {
      e.preventDefault(); const ni = Math.min(history.length - 1, histIdx + 1); setHistIdx(ni); setText(history[ni]); return;
    }
    if (e.key === "ArrowDown" && histIdx >= 0) {
      e.preventDefault(); const ni = histIdx - 1; setHistIdx(ni); setText(ni < 0 ? "" : history[ni]); return;
    }
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); send(); return; }
    if (e.key === "Enter" && !e.shiftKey && !slashOpen && !atOpen) { e.preventDefault(); send(); }
  };

  const onDrop = (e) => {
    e.preventDefault(); setDragOver(false);
    const files = [...(e.dataTransfer?.files || [])];
    if (files.length) {
      setChips(cs => [...cs, ...files.map(f => ({ id: "f-" + f.name + Date.now(), kind: "file", label: f.name, meta: (f.size/1024).toFixed(1) + "KB" }))]);
      toast && toast({ kind: "info", title: "Attached", body: `${files.length} file(s) added to context` });
    }
  };

  const riskTone = mode === "act" && !dryRun && budget > 3 ? "high" : doubt < 0.4 ? "med" : "low";

  return (
    <div className="pointer-events-auto" ref={dropRef}
         onDragOver={e => { e.preventDefault(); setDragOver(true); }}
         onDragLeave={() => setDragOver(false)}
         onDrop={onDrop}>
      <Glass className={`relative overflow-hidden px-3 py-2 transition ${focused ? "ring-1 ring-brass/30 shadow-[0_0_60px_-20px_rgba(212,175,55,0.45)]" : ""} ${dragOver ? "ring-2 ring-cyan-400/40 bg-cyan-400/[0.03]" : ""}`}>
        {/* ── Context chips (only when present) ─────────────────── */}
        {chips.length > 0 && (
          <div className="flex flex-wrap items-center gap-1.5 pb-1.5">
            <span className="font-display text-[9px] uppercase tracking-[0.22em] text-zinc-500">Context</span>
            {chips.map(c => <Chip key={c.id} chip={c} onRemove={(x) => setChips(chips.filter(y => y.id !== x.id))} />)}
          </div>
        )}

        {/* ── Prompt row ─────────────────────────────────────────── */}
        <div className="relative flex items-end gap-2">
          <button onClick={() => setChips(cs => [...cs, { id: "f-" + Date.now(), kind: "file", label: "new-file.rs", meta: "stub" }])}
                  title="Attach context (files, URLs)"
                  className="flex size-8 shrink-0 items-center justify-center rounded-md border border-white/10 bg-white/[0.02] text-zinc-400 hover:text-zinc-100 hover:border-white/25 transition">
            <Icon.plus className="size-4" />
          </button>
          <button onClick={() => setMic(m => !m)} title={mic ? "Stop dictation" : "Voice input"}
                  className={`flex size-8 shrink-0 items-center justify-center rounded-md border transition ${mic ? "border-amber-400/40 bg-amber-400/15 text-amber-300 animate-pulse" : "border-white/10 bg-white/[0.02] text-zinc-400 hover:text-zinc-100 hover:border-white/25"}`}>
            <Icon.mic className="size-4" />
          </button>

          <div className="relative flex-1">
            <textarea
              ref={taRef}
              value={text}
              onChange={e => setText(e.target.value)}
              onKeyDown={onKey}
              onFocus={() => setFocused(true)}
              onBlur={() => setTimeout(() => setFocused(false), 120)}
              rows={1}
              placeholder="Describe a task — e.g. ‘harden cryptographic invariants and add property tests’.   /  for commands,  @  for agents"
              className={`min-h-[36px] ${expanded ? "max-h-[360px]" : "max-h-[160px]"} w-full resize-none bg-transparent py-1.5 text-[14px] leading-relaxed text-zinc-100 placeholder:text-zinc-600 outline-none`}
            />

            {/* Slash command popup */}
            {slashOpen && filteredSlash.length > 0 && (
              <div className="absolute bottom-[calc(100%+6px)] left-0 z-50 w-[360px] rounded-lg border border-white/10 bg-zinc-950/95 p-1 backdrop-blur-xl shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)]">
                <div className="px-2 pt-1 pb-1.5 font-display text-[9px] uppercase tracking-[0.22em] text-zinc-500">Slash commands</div>
                {filteredSlash.map((s, i) => {
                  const IcoCmp = Icon[s.icon] || Icon.bolt;
                  return (
                    <button key={s.cmd} onMouseEnter={() => setSlashIdx(i)} onClick={() => insertSlash(s.cmd)}
                            className={`flex w-full items-center gap-2.5 rounded px-2 py-1.5 text-left ${i === slashIdx ? "bg-white/5" : ""}`}>
                      <IcoCmp className="size-3.5 text-brass" />
                      <span className="font-mono text-[11px] text-zinc-100">{s.cmd}</span>
                      <span className="ml-auto text-[10px] text-zinc-500">{s.desc}</span>
                    </button>
                  );
                })}
              </div>
            )}

            {/* @-mention popup */}
            {atOpen && filteredAt.length > 0 && (
              <div className="absolute bottom-[calc(100%+6px)] left-0 z-50 w-[280px] rounded-lg border border-white/10 bg-zinc-950/95 p-1 backdrop-blur-xl shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)]">
                <div className="px-2 pt-1 pb-1.5 font-display text-[9px] uppercase tracking-[0.22em] text-zinc-500">Mention agent</div>
                {filteredAt.map(a => (
                  <button key={a.id} onClick={() => insertAt(a)}
                          className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left hover:bg-white/5">
                    <span className="font-mono text-[10px] text-violet-300">{a.id}</span>
                    <span className="text-[11px] text-zinc-200">{a.name}</span>
                    <span className="ml-auto font-mono text-[9px] uppercase tracking-widest text-zinc-500">{a.phase}</span>
                  </button>
                ))}
              </div>
            )}
          </div>

          <button onClick={send} disabled={!text.trim()} title={text.trim() ? `${tokens} tokens${estCost != null ? ` · ~$${estCost.toFixed(3)}` : ""} · ⌘↵` : "Type a task to dispatch"}
                  className={`flex h-8 shrink-0 items-center gap-1.5 rounded-md border px-3 font-display text-[11px] uppercase tracking-[0.18em] transition ${text.trim() ? "border-brass/40 bg-brass/15 text-brass hover:bg-brass/25 shadow-[0_0_24px_-8px_rgba(212,175,55,0.6)]" : "border-white/5 bg-white/[0.02] text-zinc-600 cursor-not-allowed"}`}>
            <Icon.send className="size-3.5"/> {dryRun ? "Dry-run" : mode === "plan" ? "Plan" : mode === "verify" ? "Verify" : "Run"}
          </button>
        </div>

        {/* ── Control rail (compact, labeled) ──────────────────── */}
        <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1.5 border-t border-white/5 pt-2 text-[10px]">
          <Segment value={mode} onChange={setMode} options={LQ_MODES} />

          <div className="relative">
            <button onClick={() => { setTierOpen(o => !o); setMoreOpen(false); setSkillOpen(false); }}
                    title="Where the model runs"
                    className="inline-flex items-center gap-1 rounded-md border border-white/10 bg-white/[0.02] px-2 py-1 text-zinc-300 hover:border-white/20">
              <Icon.cpu className="size-3 text-cyan-300" /><span className="text-zinc-500">Run on</span> <span className="text-zinc-100">{tierObj.label.split(" · ")[0]}</span>
              <Icon.chev className="size-2.5 text-zinc-500" />
            </button>
            <Popover open={tierOpen}>
              <div className="px-2 pt-1 pb-1.5 font-display text-[9px] uppercase tracking-[0.22em] text-zinc-500">Where to run</div>
              {LQ_TIERS.map(t => (
                <button key={t.id} onClick={() => { setTier(t.id); setTierOpen(false); }}
                        className={`flex w-full items-start gap-2 rounded px-2 py-1.5 text-left hover:bg-white/5 ${tier === t.id ? "bg-white/5" : ""}`}>
                  <span className={`mt-0.5 size-1.5 rounded-full ${tier === t.id ? "bg-cyan-400" : "bg-white/15"}`} />
                  <div className="flex-1">
                    <div className="text-[11px] text-zinc-100">{t.label}</div>
                    <div className="font-mono text-[9px] text-zinc-500">{t.detail}</div>
                  </div>
                  <div className="text-right font-mono text-[9px] text-zinc-500"><div>{t.cost == null ? "var" : `$${t.cost.toFixed(2)}/kt`}</div><div className="text-zinc-600">{t.lat}</div></div>
                </button>
              ))}
            </Popover>
          </div>

          <div className="relative">
            <button onClick={() => { setSkillOpen(o => !o); setTierOpen(false); setMoreOpen(false); }}
                    title="Skill plugin to bias the run"
                    className="inline-flex items-center gap-1 rounded-md border border-brass/25 bg-brass/[0.06] px-2 py-1 text-brass hover:bg-brass/[0.12]">
              <Icon.bolt className="size-3" /><span className="text-brass/70">Skill</span> <span>{activeSkill ? activeSkill.name : "auto"}</span>
              <Icon.chev className="size-2.5 text-brass/60" />
            </button>
            <Popover open={skillOpen}>
              <button onClick={() => { setActiveSkill(null); setSkillOpen(false); }} className="block w-full rounded px-2 py-1.5 text-left text-[11px] text-zinc-400 hover:bg-white/5 hover:text-zinc-100">auto — let the orchestrator pick</button>
              <div className="my-1 h-px bg-white/5" />
              {skills.map(s => (
                <button key={s.id} onClick={() => { setActiveSkill(s); setSkillOpen(false); }} className={`flex w-full items-center justify-between rounded px-2 py-1.5 text-left text-[11px] hover:bg-white/5 ${activeSkill?.id === s.id ? "bg-white/5 text-brass" : "text-zinc-300"}`}>
                  <span>{s.name}</span><span className="font-mono text-[9px] uppercase tracking-widest text-zinc-500">{s.tier}</span>
                </button>
              ))}
            </Popover>
          </div>

          <span className="h-4 w-px bg-white/5" />
          <MiniSlider label="Budget"        value={budget} setValue={setBudget} min={0.25} max={20} step={0.25} fmt={v => `$${v.toFixed(2)}`} accent="#d4af37" />
          <MiniSlider label="Verify floor"  value={doubt}  setValue={setDoubt}  min={0} max={1} step={0.05} fmt={v => `${Math.round(v*100)}%`} accent="#a78bfa" />

          <span className="h-4 w-px bg-white/5" />
          <Toggle on={stream} setOn={setStream} icon="stream" label="Stream" />
          <Toggle on={dryRun} setOn={setDryRun} icon="eye"    label="Preview" />
          <Toggle on={sign}   setOn={setSign}   icon="shield" label="Sign" tone={sign ? "amber" : "zinc"} />

          <div className="relative">
            <button onClick={() => { setMoreOpen(o => !o); setTierOpen(false); setSkillOpen(false); }} className="inline-flex items-center gap-1 rounded-md border border-white/10 bg-white/[0.02] px-2 py-1 text-zinc-400 hover:text-zinc-100 hover:border-white/20">
              <Icon.more className="size-3" /> More
            </button>
            <Popover open={moreOpen}>
              <ItemRow icon="back"   label="Snapshot before run"        detail="Durable checkpoint" />
              <ItemRow icon="git"    label="Auto-branch: crypto-harden" detail="Create + switch on dispatch" />
              <ItemRow icon="clock"  label="Schedule"                   detail="Defer / cron" />
              <ItemRow icon="users"  label="Pair with reviewer"         detail="Add Socrates auditor" />
              <ItemRow icon="memory" label="Memory: project"            detail="4 corpora · vector + symbolic" />
              <ItemRow icon="alert"  label={`Risk: ${riskTone}`}        detail="mode × budget × verify" />
              <div className="my-1 h-px bg-white/5" />
              <button onClick={() => { setExpanded(e => !e); setMoreOpen(false); }} className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-[11px] text-zinc-300 hover:bg-white/5">
                <Icon.expand className="size-3" /> {expanded ? "Collapse composer" : "Expand composer"}
              </button>
            </Popover>
          </div>

          <div className="ml-auto flex items-center gap-2 font-mono text-[9px] text-zinc-500">
            {text.trim() && <span className="tabular-nums">~{tokens}t{estCost != null ? ` · $${estCost.toFixed(3)}` : ""}</span>}
            <span className={`inline-flex items-center gap-1 rounded-full border px-1.5 py-0.5 uppercase tracking-widest ${riskTone === "high" ? "border-amber-400/40 bg-amber-400/10 text-amber-300" : riskTone === "med" ? "border-violet-400/40 bg-violet-400/10 text-violet-300" : "border-emerald-400/30 bg-emerald-400/10 text-emerald-300"}`}>
              <span className={`size-1 rounded-full ${riskTone === "high" ? "bg-amber-400" : riskTone === "med" ? "bg-violet-400" : "bg-emerald-400"}`}/>{riskTone} risk
            </span>
            <kbd className="rounded border border-white/10 bg-white/5 px-1 py-0.5 tracking-widest text-zinc-400">⌘↵</kbd>
          </div>
        </div>
      </Glass>
    </div>
  );
}

function Toggle({ on, setOn, icon, label, tone = "zinc" }) {
  const IcoCmp = Icon[icon] || Icon.bolt;
  const onCls = tone === "amber" ? "border-amber-400/40 bg-amber-400/10 text-amber-300" : "border-cyan-400/30 bg-cyan-400/10 text-cyan-300";
  return (
    <button onClick={() => setOn(!on)} title={label}
            className={`inline-flex items-center gap-1 rounded-md border px-1.5 py-1 font-mono text-[10px] transition ${on ? onCls : "border-white/10 bg-white/[0.02] text-zinc-500 hover:text-zinc-200"}`}>
      <IcoCmp className="size-3" /> {label}
    </button>
  );
}

function ItemRow({ icon, label, detail }) {
  const IcoCmp = Icon[icon] || Icon.bolt;
  return (
    <button className="flex w-full items-start gap-2 rounded px-2 py-1.5 text-left hover:bg-white/5">
      <IcoCmp className="mt-0.5 size-3 text-zinc-400" />
      <div className="flex-1">
        <div className="text-[11px] text-zinc-200">{label}</div>
        <div className="font-mono text-[9px] text-zinc-500">{detail}</div>
      </div>
    </button>
  );
}

Object.assign(window, { Loquela });
