import React, { useState, useRef, useEffect, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';
import { voxTransport } from '../../../transport';
import { buildSlashEntries, type SlashEntry } from '../../../lib/slashCommands';
import {
  COMPOSER_HISTORY_CAP,
  LOQUELA_FILE_PICKER_DEBOUNCE_MS,
  LOQUELA_FILE_PICKER_LIMIT,
  LOQUELA_TIER_MODEL_COUNT,
} from '../../../config/constants';
import type { ActiveSkill, CatalogEntry, ChatPayload, Toast } from '../../../types/tauri';
import {
  formatSessionBudget,
  resolveInternalModeSlash,
} from '../../../lib/slashRouter';

const LQ_MODES = [
  { id: "plan",   label: "Plan",   hint: "Augur drafts a plan, no side effects",       tone: "text-cyan-300   border-cyan-400/30   bg-cyan-400/[0.08]" },
  { id: "act",    label: "Act",    hint: "Plan → execute under risk gates",            tone: "text-brass      border-brass/30      bg-brass/[0.08]" },
  { id: "verify", label: "Verify", hint: "Re-run with stricter doubt + property tests", tone: "text-violet-300 border-violet-400/30 bg-violet-400/[0.08]" },
];

// Static fallback tiers shown before the runtime model list loads.
// Cost is `null` (unknown) — real per-1k pricing is injected from listModels()
// once available. We never display a fabricated price.
const LQ_TIERS = [
  { id: "local", label: "Local · Mens", detail: "loading models…", cost: null, lat: null },
  { id: "mesh", label: "Mesh · Peers", detail: "peers", cost: null, lat: null },
  { id: "cloud", label: "Cloud · Cascade", detail: "cloud tier", cost: null, lat: null },
  { id: "auto", label: "Auto · Router", detail: "tier-router decides", cost: null, lat: null },
];

interface ChipData {
  id: string;
  kind: 'file' | 'skill' | 'agent' | 'branch' | 'url' | 'image';
  label: string;
  meta?: string;
}

function Chip({ chip, onRemove }: { chip: ChipData; onRemove: (c: ChipData) => void }) {
  const iconKey = { file: "file", skill: "bolt", agent: "agent", branch: "git", url: "link", image: "image" }[chip.kind] || "file";
  const IconCmp = (Icon as any)[iconKey] || Icon.file;
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

function Segment({ value, onChange, options, size = "sm" }: any) {
  const pad = size === "xs" ? "px-2 py-0.5 text-[10px]" : "px-2.5 py-1 text-[11px]";
  return (
    <div className="inline-flex items-center rounded-md border border-white/10 bg-black/30 p-0.5">
      {options.map((o: any) => {
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

function MiniSlider({ label, value, setValue, min, max, step, fmt, accent = 'rgb(var(--brass))' }: any) {
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <label className="group flex items-center gap-2 cursor-pointer">
      <span className="font-display text-[9px] uppercase tracking-[0.22em] text-zinc-500">{label}</span>
      <input type="range" min={min} max={max} step={step} value={value}
        onChange={e => setValue(parseFloat(e.target.value))}
        className="vox-range w-24 h-1 appearance-none rounded-full overflow-hidden" 
        style={{ background: `linear-gradient(to right, ${accent} ${pct}%, rgba(255,255,255,0.08) ${pct}%)` } as any} 
      />
      <span className="w-10 font-mono text-[10px] tabular-nums text-zinc-300">{fmt(value)}</span>
    </label>
  );
}

function Popover({ open, children, align = "left" }: any) {
  if (!open) return null;
  return (
    <div className={`absolute ${align === "right" ? "right-0" : "left-0"} bottom-9 z-50 min-w-[240px] rounded-lg border border-white/10 bg-zinc-950/95 p-1 backdrop-blur-xl shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)]`}>
      {children}
    </div>
  );
}

export interface SessionBudgetDisplay {
  spent: number;
  cap: number;
  source?: string;
}

export interface SlashCommandContext {
  setText: (text: string) => void;
}

interface LoquelaProps {
  chips: ChipData[];
  setChips: React.Dispatch<React.SetStateAction<ChipData[]>>;
  onSubmit: (payload: ChatPayload) => void;
  activeSkill: ActiveSkill | null;
  setActiveSkill: (s: ActiveSkill | null) => void;
  skills: CatalogEntry[];
  toast?: (t: Toast) => void;
  agents?: any[];
  sessionBudget?: SessionBudgetDisplay;
  /** When true, the slash command was handled and should not be inserted into the composer. */
  onSlashCommand?: (cmd: string, ctx: SlashCommandContext) => boolean | Promise<boolean>;
  queueDepth?: number;
  onOpenTasks?: () => void;
}

export function Loquela({
  chips,
  setChips,
  onSubmit,
  activeSkill,
  setActiveSkill,
  skills,
  toast,
  agents = [],
  sessionBudget,
  onSlashCommand,
  queueDepth,
  onOpenTasks,
}: LoquelaProps) {
  const [text, setText] = useState("");
  const [mode, setMode] = useState("act");
  const [tier, setTier] = useState("auto");
  const [dryRun, setDryRun] = useState(false);

  const [skillOpen, setSkillOpen] = useState(false);
  const [tierOpen,  setTierOpen]  = useState(false);
  const [slashOpen, setSlashOpen] = useState(false);
  const [atOpen,    setAtOpen]    = useState(false);
  const [fileSuggestions, setFileSuggestions] = useState<string[]>([]);
  const [fileSuggestionsLoading, setFileSuggestionsLoading] = useState(false);
  const filePickerDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [slashIdx,  setSlashIdx]  = useState(0);
  const [focused,   setFocused]   = useState(false);
  const [history,   setHistory]   = useState<string[]>([]);
  const [histIdx,   setHistIdx]   = useState(-1);
  const [expanded,  setExpanded]  = useState(false);
  const [runtimeTiers, setRuntimeTiers] = useState(LQ_TIERS);

  const taRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const ta = taRef.current; if (!ta) return;
    ta.style.height = "auto";
    ta.style.height = Math.min(expanded ? 360 : 200, ta.scrollHeight) + "px";
  }, [text, expanded]);

  useEffect(() => {
    const trimmed = text.trimStart();
    setSlashOpen(trimmed.startsWith("/") && !trimmed.includes(" "));
    const m = text.match(/@([^\s]*)$/);
    setAtOpen(!!m);
    setSlashIdx(0);
  }, [text]);

  const atQuery = useMemo(() => {
    const m = text.match(/@([^\s]*)$/);
    return m?.[1] ?? "";
  }, [text]);

  const isPathLikeAtQuery = (q: string) =>
    q.length === 0 || /[./\\-]/.test(q) || q.includes("/");

  useEffect(() => {
    if (!atOpen) {
      setFileSuggestions([]);
      setFileSuggestionsLoading(false);
      if (filePickerDebounceRef.current) clearTimeout(filePickerDebounceRef.current);
      return;
    }
    if (filePickerDebounceRef.current) clearTimeout(filePickerDebounceRef.current);
    filePickerDebounceRef.current = setTimeout(async () => {
      setFileSuggestionsLoading(true);
      try {
        const paths = await invoke<string[]>("list_repo_files", {
          query: atQuery || null,
          limit: LOQUELA_FILE_PICKER_LIMIT,
        });
        setFileSuggestions(Array.isArray(paths) ? paths : []);
      } catch {
        setFileSuggestions([]);
      } finally {
        setFileSuggestionsLoading(false);
      }
    }, LOQUELA_FILE_PICKER_DEBOUNCE_MS);
    return () => {
      if (filePickerDebounceRef.current) clearTimeout(filePickerDebounceRef.current);
    };
  }, [atOpen, atQuery]);

  // Refresh the model/tier list on focus + every 60s so it never goes stale (C2).
  useEffect(() => {
    let cancelled = false;
    let firstLoad = true;
    const loadTiers = () => {
      voxTransport.listModels(LOQUELA_TIER_MODEL_COUNT).then((models: any) => {
        if (cancelled || !Array.isArray(models) || models.length === 0) return;
        const dynamic = models.slice(0, 4).map((m: any, idx: number) => {
          const perK = typeof m.cost_per_1k === 'number' ? m.cost_per_1k : null;
          return {
            id: m.model_id ?? `model-${idx}`,
            label: m.display_name ?? m.model_id ?? `Model ${idx + 1}`,
            detail: m.provider ?? 'runtime',
            cost: perK,
            lat: null,
          };
        });
        setRuntimeTiers([
          ...dynamic,
          { id: 'auto', label: 'Auto · Router', detail: 'live routing summary', cost: null, lat: null },
        ]);
        if (firstLoad) {
          firstLoad = false;
          if (!dynamic.some((d: any) => d.id === tier) && tier !== 'auto') {
            setTier(dynamic[0]?.id ?? 'auto');
          }
        }
      }).catch(() => {});
    };
    loadTiers();
    const interval = setInterval(loadTiers, 60_000);
    const onFocus = () => loadTiers();
    window.addEventListener('focus', onFocus);
    return () => {
      cancelled = true;
      clearInterval(interval);
      window.removeEventListener('focus', onFocus);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const allSlash = useMemo(() => buildSlashEntries(skills), [skills]);
  const filteredSlash = useMemo(() => {
    const q = text.trimStart().toLowerCase();
    return allSlash.filter(s => s.cmd.startsWith(q));
  }, [text, allSlash]);

  const filteredAt = useMemo(() => {
    const q = atQuery.toLowerCase();
    return agents.filter(a => a.id.toLowerCase().includes(q) || a.codename?.toLowerCase().includes(q));
  }, [atQuery, agents]);

  const showFileSuggestions = atOpen && (isPathLikeAtQuery(atQuery) || fileSuggestions.length > 0);
  const showAtPopover = atOpen && (filteredAt.length > 0 || showFileSuggestions || fileSuggestionsLoading);

  const tokens = Math.ceil(text.length / 4) + chips.length * 80;
  const tierObj = runtimeTiers.find(t => t.id === tier) || runtimeTiers[runtimeTiers.length - 1];
  const estCost =
    tierObj?.cost == null ? null : (tokens / 1000) * tierObj.cost;

  const [recording, setRecording] = useState(false);
  const [transcribing, setTranscribing] = useState(false);

  // Add a single locator (file path or URL) to the shared context set as a chip.
  // These chips become the next task's file manifest (App.handleLoquelaSubmit).
  const addContextRef = (ref: string) => {
    const trimmed = ref.trim();
    if (!trimmed) return;
    const isUrl = /^https?:\/\//i.test(trimmed);
    const id = `ctx-${isUrl ? 'url' : 'file'}-${trimmed}`;
    setChips(cs => cs.find(c => c.id === id)
      ? cs
      : [...cs, { id, kind: isUrl ? 'url' : 'file', label: trimmed }]);
  };

  // Attach local files to the task context via the native OS file-picker
  // (tauri-plugin-dialog). Each chosen path flows to the orchestrator as a
  // FileAffinity through the shared Loquela context set. Falls back to a typed
  // path/URL prompt when the dialog is unavailable (e.g. browser dev mode).
  const attachContext = async () => {
    try {
      const selected = await openFileDialog({ multiple: true, title: 'Attach files to task context' });
      const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
      if (paths.length) {
        paths.forEach(addContextRef);
        toast?.({ tone: 'ok', title: paths.length === 1 ? 'Attached to context' : `${paths.length} attached`, body: paths.join(', '), cmd: 'context.attach' });
        taRef.current?.focus();
        return;
      }
      if (selected !== null) return; // dialog opened, user picked nothing
    } catch {
      // Dialog plugin unavailable → fall through to the prompt path.
    }
    const raw = window.prompt('Attach a file path or URL to this task context:');
    const ref = raw?.trim();
    if (!ref) return;
    addContextRef(ref);
    toast?.({ tone: 'ok', title: 'Attached to context', body: ref, cmd: 'context.attach' });
    taRef.current?.focus();
  };

  // Attach a URL (the native picker only handles local files).
  const attachUrl = () => {
    const raw = window.prompt('Attach a URL to this task context:');
    const ref = raw?.trim();
    if (!ref) return;
    addContextRef(ref);
    toast?.({ tone: 'ok', title: 'Attached to context', body: ref, cmd: 'context.attach' });
    taRef.current?.focus();
  };

  // Microphone capture → on-device transcription. Toggles record/stop; on stop,
  // appends the refined transcript into the composer textarea.
  const toggleMic = async () => {
    if (transcribing) return;
    if (!recording) {
      try {
        await invoke('start_mic_capture');
        setRecording(true);
        toast?.({ tone: 'info', title: 'Recording', body: 'Listening — tap the mic again to stop.', cmd: 'oratio.transcribe' });
      } catch (e) {
        toast?.({ tone: 'warn', title: 'Microphone unavailable', body: String(e), cmd: 'oratio.transcribe' });
      }
      return;
    }
    // Stop + transcribe.
    setRecording(false);
    setTranscribing(true);
    try {
      const transcript = await invoke<string>('stop_mic_capture_and_transcribe');
      const t = (transcript || '').trim();
      if (t) {
        setText(prev => (prev ? `${prev.replace(/\s*$/, '')} ${t}` : t));
        taRef.current?.focus();
        toast?.({ tone: 'ok', title: 'Transcribed', body: t, cmd: 'oratio.transcribe' });
      } else {
        toast?.({ tone: 'info', title: 'No speech detected', body: 'The recording produced no transcript.', cmd: 'oratio.transcribe' });
      }
    } catch (e) {
      toast?.({ tone: 'warn', title: 'Transcription failed', body: String(e), cmd: 'oratio.transcribe' });
    } finally {
      setTranscribing(false);
    }
  };

  const runSlash = async (entry: SlashEntry) => {
    // Skill entries pin the skill (rides in the payload's active_skill) rather
    // than inserting literal text.
    if (entry.kind === 'skill') {
      const found = skills.find(
        (s) => (s.capability_id ?? s.command) === entry.skillId || s.command === entry.skillId,
      );
      const name = entry.cmd.slice(1);
      setActiveSkill(
        found
          ? { id: found.capability_id ?? found.command, name: found.command, command: found.command }
          : { id: entry.skillId ?? name, name, command: name },
      );
      setText('');
      setSlashOpen(false);
      toast?.({ tone: 'info', title: 'Skill selected', body: name, cmd: 'skill.pin' });
      taRef.current?.focus();
      return;
    }
    const cmd = entry.cmd;
    // Internal mode slashes (/plan, /verify, …) flip the composer mode.
    const internalMode = resolveInternalModeSlash(cmd);
    if (internalMode) {
      setMode(internalMode);
      setText('');
      setSlashOpen(false);
      const hint = LQ_MODES.find(m => m.id === internalMode)?.hint;
      toast?.({ tone: 'info', title: `${internalMode} mode`, body: hint, cmd });
      taRef.current?.focus();
      return;
    }
    const handled = await onSlashCommand?.(cmd, { setText });
    if (handled) {
      setText('');
      setSlashOpen(false);
      taRef.current?.focus();
      return;
    }
    setText(cmd + ' ');
    setSlashOpen(false);
    taRef.current?.focus();
  };
  const insertAt = (agent: any) => {
    setText(t => t.replace(/@[^\s]*$/, `@${agent.id} `));
    setChips(cs => cs.find(c => c.id === "agent-" + agent.id) ? cs : [...cs, { id: "agent-" + agent.id, kind: "agent", label: `${agent.id} · ${agent.codename}`, meta: agent.phase }]);
    setAtOpen(false); taRef.current?.focus();
  };

  const insertAtFile = (path: string) => {
    setText(t => t.replace(/@[^\s]*$/, `@${path} `));
    addContextRef(path);
    setAtOpen(false);
    taRef.current?.focus();
  };

  const send = () => {
    if (!text.trim()) return;
    const payload = {
      description: text.trim(),
      active_skill: activeSkill?.id,
      mode,
      tier,
      dry_run: dryRun,
      context: chips.map(c => ({ kind: c.kind, ref: c.label })),
    };
    onSubmit(payload);
    setHistory(h => [text.trim(), ...h].slice(0, COMPOSER_HISTORY_CAP));
    setHistIdx(-1);
    setText("");
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (slashOpen && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      e.preventDefault();
      setSlashIdx(i => (i + (e.key === "ArrowDown" ? 1 : -1) + filteredSlash.length) % Math.max(1, filteredSlash.length));
      return;
    }
    if (slashOpen && e.key === "Enter") { e.preventDefault(); const s = filteredSlash[slashIdx]; if (s) void runSlash(s); return; }
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

  const riskTone = dryRun ? 'low' : mode === 'verify' ? 'med' : mode === 'plan' ? 'low' : 'high';

  return (
    <div className="pointer-events-auto p-4">
      <Glass className={`relative overflow-hidden px-3 py-2 transition ${focused ? "ring-1 ring-brass/30 shadow-[0_0_60px_-20px_rgb(var(--brass)_/_0.45)]" : ""}`}>
        {chips.length > 0 && (
          <div className="flex flex-wrap items-center gap-1.5 pb-1.5">
            <span className="font-display text-[9px] uppercase tracking-[0.22em] text-zinc-500">Context</span>
            {chips.map(c => <Chip key={c.id} chip={c} onRemove={(x) => setChips(chips.filter(y => y.id !== x.id))} />)}
          </div>
        )}

        <div className="relative flex items-end gap-2">
          <button onClick={attachContext} title="Attach local file(s) to context (native picker)" className="flex size-8 shrink-0 items-center justify-center rounded-md border border-white/10 bg-white/[0.02] text-zinc-400 hover:text-zinc-100 hover:border-white/25 transition">
            <Icon.plus className="size-4" />
          </button>
          <button onClick={attachUrl} title="Attach a URL to context" className="flex size-8 shrink-0 items-center justify-center rounded-md border border-white/10 bg-white/[0.02] text-zinc-400 hover:text-zinc-100 hover:border-white/25 transition">
            <Icon.link className="size-4" />
          </button>
          <button
            onClick={toggleMic}
            disabled={transcribing}
            title={transcribing ? 'Transcribing…' : recording ? 'Stop recording & transcribe' : 'Voice input — record & transcribe'}
            aria-pressed={recording}
            className={`flex size-8 shrink-0 items-center justify-center rounded-md border transition ${
              transcribing
                ? 'border-white/10 bg-white/[0.02] text-zinc-500 cursor-wait'
                : recording
                ? 'border-rose-400/50 bg-rose-400/15 text-rose-300 animate-pulse'
                : 'border-white/10 bg-white/[0.02] text-zinc-400 hover:text-zinc-100 hover:border-white/25'
            }`}
          >
            <Icon.mic className="size-4" />
          </button>

          <div className="relative flex-1">
            <textarea
              id="loquela-composer"
              ref={taRef}
              value={text}
              aria-label="Task composer"
              onChange={e => setText(e.target.value)}
              onKeyDown={onKey}
              onFocus={() => setFocused(true)}
              onBlur={() => setTimeout(() => setFocused(false), 120)}
              rows={1}
              placeholder="Describe a task — e.g. ‘harden cryptographic invariants’. / for commands, @ for agents or files"
              className={`min-h-[36px] ${expanded ? "max-h-[360px]" : "max-h-[160px]"} w-full resize-none bg-transparent py-1.5 text-[14px] leading-relaxed text-zinc-100 placeholder:text-zinc-600 outline-none`}
            />

            {slashOpen && filteredSlash.length > 0 && (
              <div className="absolute bottom-[calc(100%+6px)] left-0 z-50 w-[360px] rounded-lg border border-white/10 bg-zinc-950/95 p-1 backdrop-blur-xl shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)]">
                <div className="px-2 pt-1 pb-1.5 font-display text-[9px] uppercase tracking-[0.22em] text-zinc-500">Slash commands</div>
                {filteredSlash.map((s, i) => {
                  const IcoCmp = (Icon as any)[s.icon] || Icon.bolt;
                  return (
                    <button key={s.cmd} onMouseEnter={() => setSlashIdx(i)} onClick={() => void runSlash(s)}
                            className={`flex w-full items-center gap-2.5 rounded px-2 py-1.5 text-left ${i === slashIdx ? "bg-white/5" : ""}`}>
                      <IcoCmp className="size-3.5 text-brass" />
                      <span className="font-mono text-[11px] text-zinc-100">{s.cmd}</span>
                      <span className="ml-auto text-[10px] text-zinc-500">{s.desc}</span>
                    </button>
                  );
                })}
              </div>
            )}

            {showAtPopover && (
              <div className="absolute bottom-[calc(100%+6px)] left-0 z-50 w-[360px] max-h-[280px] overflow-y-auto rounded-lg border border-white/10 bg-zinc-950/95 p-1 backdrop-blur-xl shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)]">
                {filteredAt.length > 0 && (
                  <>
                    <div className="px-2 pt-1 pb-1.5 font-display text-[9px] uppercase tracking-[0.22em] text-zinc-500">Agents</div>
                    {filteredAt.map(a => (
                      <button key={a.id} onClick={() => insertAt(a)}
                              className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left hover:bg-white/5">
                        <span className="font-mono text-[10px] text-violet-300">{a.id}</span>
                        <span className="text-[11px] text-zinc-200">{a.codename}</span>
                        <span className="ml-auto font-mono text-[9px] uppercase tracking-widest text-zinc-500">{a.phase}</span>
                      </button>
                    ))}
                  </>
                )}
                {showFileSuggestions && (
                  <>
                    <div className={`px-2 pt-1 pb-1.5 font-display text-[9px] uppercase tracking-[0.22em] text-zinc-500 ${filteredAt.length > 0 ? "mt-1 border-t border-white/5" : ""}`}>
                      Files
                    </div>
                    {fileSuggestionsLoading && fileSuggestions.length === 0 && (
                      <div className="px-2 py-1.5 text-[10px] text-zinc-500">Searching repo…</div>
                    )}
                    {fileSuggestions.map(p => (
                      <button key={p} onClick={() => insertAtFile(p)}
                              className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left hover:bg-white/5">
                        <Icon.file className="size-3 shrink-0 text-cyan-300" />
                        <span className="truncate font-mono text-[10px] text-zinc-200">{p}</span>
                      </button>
                    ))}
                    {!fileSuggestionsLoading && fileSuggestions.length === 0 && (
                      <div className="px-2 py-1.5 text-[10px] text-zinc-500">No matching files</div>
                    )}
                  </>
                )}
              </div>
            )}
          </div>

          <button onClick={send} disabled={!text.trim()} className={`flex h-8 shrink-0 items-center gap-1.5 rounded-md border px-3 font-display text-[11px] uppercase tracking-[0.18em] transition ${text.trim() ? "border-brass/40 bg-brass/15 text-brass hover:bg-brass/25 shadow-[0_0_24px_-8px_rgb(var(--brass)_/_0.6)]" : "border-white/5 bg-white/[0.02] text-zinc-600 cursor-not-allowed"}`}>
            <Icon.send className="size-3.5"/> {dryRun ? "Dry-run" : mode === "plan" ? "Plan" : mode === "verify" ? "Verify" : "Run"}
          </button>
        </div>

        <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1.5 border-t border-white/5 pt-2 text-[10px]">
          <Segment value={mode} onChange={setMode} options={LQ_MODES} />

          {typeof queueDepth === 'number' && queueDepth > 0 && (
            <button
              onClick={onOpenTasks}
              title="Open task list"
              className="flex items-center gap-1 rounded-full border border-brass/25 bg-brass/10 px-2 py-0.5 font-mono text-[10px] text-brass hover:bg-brass/20 focus:outline-none focus:ring-1 focus:ring-brass/40"
            >
              {queueDepth} queued
            </button>
          )}

          <div className="relative">
            <button onClick={() => { setTierOpen(o => !o); setSkillOpen(false); }} className="inline-flex items-center gap-1 rounded-md border border-white/10 bg-white/[0.02] px-2 py-1 text-zinc-300 hover:border-white/20">
              <Icon.cpu className="size-3 text-cyan-300" /><span className="text-zinc-500">Run on</span> <span className="text-zinc-100">{tierObj.label.split(" · ")[0]}</span>
              <Icon.chevR className="size-2.5 text-zinc-500 rotate-90" />
            </button>
            <Popover open={tierOpen}>
              {runtimeTiers.map(t => (
                <button key={t.id} onClick={() => { setTier(t.id); setTierOpen(false); }} className={`flex w-full items-start gap-2 rounded px-2 py-1.5 text-left hover:bg-white/5 ${tier === t.id ? "bg-white/5" : ""}`}>
                  <div className="flex-1">
                    <div className="text-[11px] text-zinc-100">{t.label}</div>
                    <div className="font-mono text-[9px] text-zinc-500">{t.detail}</div>
                  </div>
                </button>
              ))}
            </Popover>
          </div>

          <div className="relative">
            <button onClick={() => { setSkillOpen(o => !o); setTierOpen(false); }} className="inline-flex items-center gap-1 rounded-md border border-brass/25 bg-brass/[0.06] px-2 py-1 text-brass hover:bg-brass/[0.12]">
              <Icon.bolt className="size-3" /><span className="text-brass/70">Skill</span> <span>{activeSkill ? (activeSkill.name ?? activeSkill.command ?? activeSkill.id) : "auto"}</span>
              <Icon.chevR className="size-2.5 text-brass/60 rotate-90" />
            </button>
            <Popover open={skillOpen}>
              <button onClick={() => { setActiveSkill(null); setSkillOpen(false); }} className="block w-full rounded px-2 py-1.5 text-left text-[11px] text-zinc-400 hover:bg-white/5 hover:text-zinc-100">auto</button>
              {skills.map(s => {
                const skillId = s.capability_id ?? s.command;
                return (
                <button key={skillId} onClick={() => { setActiveSkill({ id: skillId, name: s.command, command: s.command }); setSkillOpen(false); }} className={`flex w-full items-center justify-between rounded px-2 py-1.5 text-left text-[11px] hover:bg-white/5 ${activeSkill?.id === skillId ? "bg-white/5 text-brass" : "text-zinc-300"}`}>
                  <span>{s.command}</span>
                </button>
              );})}
            </Popover>
          </div>

          
          {(estCost != null || sessionBudget) && (
            <span className="font-mono text-[9px] text-zinc-500 tabular-nums">
              {estCost != null && (
                <>~{tokens} tok · ~${estCost.toFixed(3)}</>
              )}
              {estCost != null && sessionBudget && sessionBudget.cap > 0 && ' · '}
              {sessionBudget && sessionBudget.cap > 0 && (
                <>{formatSessionBudget(sessionBudget.spent, sessionBudget.cap)}</>
              )}
            </span>
          )}

          <div className="ml-auto flex items-center gap-2 font-mono text-[9px] text-zinc-500">
            <span className={`inline-flex items-center gap-1 rounded-full border px-1.5 py-0.5 uppercase tracking-widest ${riskTone === "high" ? "border-amber-400/40 bg-amber-400/10 text-amber-300" : riskTone === "med" ? "border-violet-400/40 bg-violet-400/10 text-violet-300" : "border-emerald-400/30 bg-emerald-400/10 text-emerald-300"}`}>
              {riskTone} risk
            </span>
            <kbd className="rounded border border-white/10 bg-white/5 px-1 py-0.5 tracking-widest text-zinc-400">⌘↵</kbd>
          </div>
        </div>
      </Glass>
    </div>
  );
}
