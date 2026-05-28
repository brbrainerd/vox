import React, { useState } from 'react';
import { Glass } from '../../ui/Glass';
import { invoke } from '@tauri-apps/api/core';
import { Icon } from '../../ui/Icons';

const SECTIONS = [
  { id: 'orchestrator', icon: 'cpu',     label: 'Orchestrator' },
  { id: 'mesh',         icon: 'flow',    label: 'Mesh & peers' },
  { id: 'signing',      icon: 'shield',  label: 'Signing keys' },
  { id: 'telemetry',    icon: 'scale',   label: 'Telemetry' },
  { id: 'keybinds',     icon: 'command', label: 'Keybinds' },
  { id: 'theme',        icon: 'spark',   label: 'Theme' },
];

const KEYBINDS = [
  ['⌘K',   'Open command palette'],
  ['⌘↵',  'Dispatch intent'],
  ['⇧↵',  'Newline in composer'],
  ['/',     'Slash command'],
  ['@',     'Mention agent'],
  ['↑/↓', 'History recall'],
  ['⌘B',   'Toggle sidebar'],
  ['⌘.',   'Pause/resume selected agent'],
];

interface SettingsState {
  doubt: boolean;
  autobudget: boolean;
  theme: string;
  concurrency: number;
  capUsd: number;
  doubtThresh: number;
  sign: boolean;
  telemetry: string;
  isolation: string;
  checkpointMins: number;
}

function Row({ label, hint, children }: { label: string; hint: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-xl border border-white/5 bg-white/[0.02] p-3">
      <div>
        <div className="font-display text-[12px] text-zinc-200">{label}</div>
        <div className="text-[11px] text-zinc-500">{hint}</div>
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function Toggle({ on, onClick }: { on: boolean; onClick: () => void }) {
  return (
    <button onClick={onClick} className={`relative h-5 w-9 rounded-full transition ${on ? 'bg-brass/40' : 'bg-white/10'}`}>
      <span className={`absolute top-0.5 size-4 rounded-full bg-zinc-50 transition ${on ? 'left-[18px]' : 'left-0.5'}`} />
    </button>
  );
}

function RangeInline({
  value, min, max, step = 1, suffix = '', onChange,
}: {
  value: number; min: number; max: number; step?: number; suffix?: string; onChange: (v: number) => void;
}) {
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <div className="flex w-52 items-center gap-3">
      <input
        type="range" min={min} max={max} step={step} value={value}
        onChange={e => onChange(Number(e.target.value))}
        className="vox-range flex-1 h-1 appearance-none rounded-full overflow-hidden"
        style={{ background: `linear-gradient(to right, #d4af37 ${pct}%, rgba(255,255,255,0.08) ${pct}%)` } as any}
      />
      <span className="w-14 text-right font-mono text-[11px] text-zinc-200">{suffix}{value}</span>
    </div>
  );
}

const MOCK_PEERS = [
  { name: 'forge.local',  backend: 'candle-cuda', vram: '24GB', online: true,  trust: 'verified' },
  { name: 'oracle.local', backend: 'mlx-metal',   vram: '36GB', online: true,  trust: 'verified' },
  { name: 'node-03',      backend: 'vllm-cuda',   vram: '40GB', online: true,  trust: 'pending' },
  { name: 'node-04',      backend: 'candle-cpu',  vram: '—',    online: false, trust: 'revoked' },
];

const MOCK_KEYS = [
  { id: 'clavis-primary',  fp: 'ed25519:7F:42:9B…2A:11', rotated: '4d ago',  scope: 'all' },
  { id: 'clavis-readonly', fp: 'ed25519:11:CD:8E…77:0A', rotated: '22d ago', scope: 'recall-only' },
];

interface SettingsViewProps {
  pushToast: (t: any) => void;
}

export function SettingsView({ pushToast }: SettingsViewProps) {
  const [section, setSection] = useState('orchestrator');
  const [vals, setVals] = useState<SettingsState>({
    doubt: true, autobudget: true, theme: 'arcane', concurrency: 7,
    capUsd: 5, doubtThresh: 0.6, sign: false, telemetry: 'local',
    isolation: 'wasm', checkpointMins: 5,
  });

  const update = async (patch: Partial<SettingsState>) => {
    const next = { ...vals, ...patch };
    setVals(next);
    
    // Attempt to push to Rust (fails gracefully if command not registered)
    try {
      await invoke('set_orchestrator_config', { config: next });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Save failed', body: String(err) });
    }
  };

  return (
    <div className="grid grid-cols-12 gap-5">
      {/* Nav */}
      <Glass className="col-span-12 md:col-span-3 p-3">
        <nav className="flex flex-col gap-1">
          {SECTIONS.map(s => {
            const IcoCmp = (Icon as any)[s.icon] ?? Icon.bolt;
            const on = section === s.id;
            return (
              <button
                key={s.id}
                onClick={() => setSection(s.id)}
                className={`flex items-center gap-2.5 rounded-lg px-3 py-2 text-left transition ${
                  on ? 'bg-white/[0.05] text-zinc-100' : 'text-zinc-400 hover:bg-white/[0.025] hover:text-zinc-200'
                }`}
              >
                <IcoCmp className={`size-4 ${on ? 'text-brass' : 'text-zinc-500'}`} />
                <span className="font-display text-[12px] tracking-[0.12em] uppercase">{s.label}</span>
              </button>
            );
          })}
        </nav>
      </Glass>

      {/* Content */}
      <Glass className="col-span-12 md:col-span-9 p-5">
        {section === 'orchestrator' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Orchestrator</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">Global scheduling, budget, and verification policy</p>
            <div className="mt-4 space-y-3">
              <Row label="Max concurrent agents" hint="Hard cap before scheduler back-pressure">
                <RangeInline value={vals.concurrency} min={1} max={16} onChange={v => update({ concurrency: v })} />
              </Row>
              <Row label="Global budget cap (USD)" hint="Soft + hard cap. Throttles when reached.">
                <RangeInline value={vals.capUsd} min={1} max={50} step={1} suffix="$" onChange={v => update({ capUsd: v })} />
              </Row>
              <Row label="Auto-doubt threshold" hint="Confidence floor below which Augur intervenes">
                <RangeInline value={Math.round(vals.doubtThresh * 100)} min={0} max={100} step={5} suffix="%" onChange={v => update({ doubtThresh: v / 100 })} />
              </Row>
              <Row label="Durable checkpoint cadence" hint="Snapshot interval for resumable runs">
                <RangeInline value={vals.checkpointMins} min={1} max={30} step={1} suffix=" min" onChange={v => update({ checkpointMins: v })} />
              </Row>
              <Row label="Default isolation tier" hint="Runtime sandbox for new agents">
                <div className="inline-flex items-center rounded-md border border-white/10 bg-black/30 p-0.5">
                  {([['wasm', 'WASM'], ['ctr', 'Container'], ['native', 'Native']] as [string, string][]).map(([id, l]) => (
                    <button
                      key={id}
                      onClick={() => update({ isolation: id })}
                      className={`rounded-[5px] px-2 py-1 font-display text-[10px] uppercase tracking-[0.15em] transition ${
                        vals.isolation === id ? 'bg-white/10 text-zinc-50' : 'text-zinc-500 hover:text-zinc-300'
                      }`}
                    >
                      {l}
                    </button>
                  ))}
                </div>
              </Row>
              <Row label="Auto-doubt unverified outputs" hint="Inject Augur into the verify lane">
                <Toggle on={vals.doubt} onClick={() => update({ doubt: !vals.doubt })} />
              </Row>
              <Row label="Auto-budget per agent" hint="Derived from skill + recent burn">
                <Toggle on={vals.autobudget} onClick={() => update({ autobudget: !vals.autobudget })} />
              </Row>
            </div>
          </>
        )}

        {section === 'mesh' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Mesh & peers</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">Discover and authorise peer compute on the local mesh</p>
            <div className="mt-4 space-y-2">
              {MOCK_PEERS.map(p => (
                <div key={p.name} className="flex items-center justify-between rounded-md border border-white/5 bg-white/[0.02] p-3">
                  <div className="flex items-center gap-3">
                    <span className={`size-2 rounded-full ${p.online ? 'bg-emerald-400' : 'bg-zinc-600'}`} />
                    <div className="leading-tight">
                      <div className="font-mono text-[12px] text-zinc-100">{p.name}</div>
                      <div className="font-mono text-[10px] text-zinc-500">{p.backend} · {p.vram}</div>
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className={`rounded-full px-2 py-0.5 font-display text-[9px] uppercase tracking-widest ${
                      p.trust === 'verified' ? 'bg-emerald-400/15 text-emerald-300' :
                      p.trust === 'pending'  ? 'bg-amber-400/15 text-amber-300' :
                                               'bg-zinc-700/40 text-zinc-400'
                    }`}>{p.trust}</span>
                    <button
                      onClick={() => pushToast({ tone: 'ok', title: `${p.name} action`, cmd: 'mesh.toggle_trust' })}
                      className="rounded border border-white/10 bg-white/[0.02] px-2 py-1 font-mono text-[10px] text-zinc-300 hover:bg-white/5"
                    >manage</button>
                  </div>
                </div>
              ))}
            </div>
          </>
        )}

        {section === 'signing' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Signing keys</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">ed25519 capability gates for high-risk dispatch</p>
            <div className="mt-4 space-y-2">
              {MOCK_KEYS.map(k => (
                <div key={k.id} className="flex items-center justify-between rounded-md border border-white/5 bg-white/[0.02] p-3">
                  <div className="flex items-center gap-3">
                    <Icon.shield className="size-4 text-amber-300" />
                    <div className="leading-tight">
                      <div className="font-mono text-[12px] text-zinc-100">{k.id}</div>
                      <div className="font-mono text-[10px] text-zinc-500">{k.fp} · rotated {k.rotated}</div>
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="rounded-full bg-white/[0.04] px-2 py-0.5 font-display text-[9px] uppercase tracking-widest text-zinc-300">{k.scope}</span>
                    <button
                      onClick={() => pushToast({ tone: 'warn', title: 'Rotation queued', body: k.id, cmd: 'clavis.rotate' })}
                      className="rounded border border-white/10 bg-white/[0.02] px-2 py-1 font-mono text-[10px] text-zinc-300 hover:bg-white/5"
                    >rotate</button>
                  </div>
                </div>
              ))}
              <Row label="Require signature on Native isolation" hint="Hard gate; refuses dispatch without ed25519">
                <Toggle on={vals.sign} onClick={() => update({ sign: !vals.sign })} />
              </Row>
            </div>
          </>
        )}

        {section === 'telemetry' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Telemetry</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">Where Vox sends spans, metrics, and traces</p>
            <div className="mt-4 grid grid-cols-3 gap-2">
              {([['off', 'Off', 'Nothing leaves the device'], ['local', 'Local', 'OTLP → localhost:4317'], ['cloud', 'Cloud', 'Encrypted → vendor']] as [string, string, string][]).map(([id, l, h]) => (
                <button
                  key={id}
                  onClick={() => update({ telemetry: id })}
                  className={`rounded-xl border p-3 text-left transition ${
                    vals.telemetry === id ? 'border-brass/40 bg-brass/[0.05]' : 'border-white/5 hover:border-white/15 bg-white/[0.02]'
                  }`}
                >
                  <div className="font-display text-[12px] tracking-wider text-zinc-100">{l}</div>
                  <div className="mt-1 font-mono text-[10px] text-zinc-500">{h}</div>
                </button>
              ))}
            </div>
          </>
        )}

        {section === 'keybinds' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Keybinds</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">Global shortcuts</p>
            <div className="mt-4 grid grid-cols-1 gap-1.5 md:grid-cols-2">
              {KEYBINDS.map(([k, d]) => (
                <div key={k} className="flex items-center justify-between rounded-md border border-white/5 bg-white/[0.02] px-3 py-2">
                  <span className="text-[12px] text-zinc-200">{d}</span>
                  <kbd className="rounded border border-white/10 bg-white/5 px-2 py-0.5 font-mono text-[10px] text-zinc-300">{k}</kbd>
                </div>
              ))}
            </div>
          </>
        )}

        {section === 'theme' && (
          <>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Theme</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">Aesthetic mode for the GUI layer</p>
            <div className="mt-4 grid grid-cols-2 gap-3 md:grid-cols-3">
              {[
                { id: 'arcane',  name: 'Arcane',  swatch: 'from-brass via-amber-600 to-zinc-900' },
                { id: 'void',    name: 'Void',    swatch: 'from-violet-500 via-zinc-800 to-zinc-950' },
                { id: 'glacier', name: 'Glacier', swatch: 'from-cyan-400 via-slate-700 to-zinc-950' },
              ].map(t => (
                <button
                  key={t.id}
                  onClick={() => update({ theme: t.id })}
                  className={`rounded-xl border p-3 text-left transition ${
                    vals.theme === t.id ? 'border-brass/40 bg-brass/[0.05]' : 'border-white/5 hover:border-white/15 bg-white/[0.02]'
                  }`}
                >
                  <div className={`h-16 w-full rounded-lg bg-gradient-to-br ${t.swatch}`} />
                  <div className="mt-2 font-display text-[12px] tracking-wider text-zinc-200">{t.name}</div>
                </button>
              ))}
            </div>
          </>
        )}
      </Glass>
    </div>
  );
}
