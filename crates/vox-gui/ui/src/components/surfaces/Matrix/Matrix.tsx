import React, { useCallback, useEffect, useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { Pill, PHASE_TONE } from '../../ui/Pill';
import { phaseFill, phaseStroke } from '../../../lib/visualTokens';
import { useLabel } from '../../../hooks/useLanguage';
import { MATRIX_POLL_MS } from '../../../config/constants';
import { recordGamifyGuiEvent } from '../../../lib/gamifyGuiEvents';
import { useIsEmbeddedSurface } from '../../dashboard/EmbeddedSurfaceContext';

/** One routing-priority axis projected onto the hex grid (mirrors the Rust
 *  `RoutingIntentionDto`). */
interface RoutingIntention {
  id: string;
  parent: string;
  branch: string;
  phase: string;
  conf: number;
  note: string;
}

function HexCell({ intention, onSelect, selected }: { intention: RoutingIntention; onSelect: (id: string) => void; selected: boolean }) {
  const conf = intention.conf;
  const stroke = phaseStroke(intention.phase === 'Active' ? 'Active' : intention.phase);
  const phaseTone = {
    stroke,
    fill: phaseFill(stroke, conf),
    text: (PHASE_TONE as Record<string, { text: string }>)[intention.phase]?.text ?? PHASE_TONE.Active.text,
    glow: stroke,
  };

  return (
    <button
      type="button"
      onClick={() => onSelect(intention.id)}
      aria-pressed={selected}
      aria-label={`${intention.branch} routing axis (${intention.parent}, ${Math.round(conf * 100)}% weight, ${intention.phase})`}
      className="group relative aspect-[1/1.05] [clip-path:polygon(50%_0,100%_25%,100%_75%,50%_100%,0_75%,0_25%)] focus:outline-hidden"
      style={{ background: phaseTone.fill, boxShadow: `inset 0 0 0 1px ${phaseTone.stroke}40` }}
    >
      <div className="absolute inset-0 [clip-path:polygon(50%_0,100%_25%,100%_75%,50%_100%,0_75%,0_25%)] opacity-60" style={{ background: `radial-gradient(circle at center, ${phaseTone.glow}33, transparent 70%)` }} />
      <div className="relative flex h-full flex-col items-center justify-center px-4 text-center">
        <div className="font-mono text-[9px] uppercase tracking-[0.2em] text-text-muted">{intention.parent}</div>
        <div className={`mt-1 font-display text-[13px] font-semibold tracking-tight ${phaseTone.text}`}>{intention.branch}</div>
        <div className="mt-1.5 font-display text-[22px] font-bold tabular-nums text-text-primary">{Math.round(conf*100)}<span className="text-[12px] text-text-muted">%</span></div>
      </div>
      {selected && (
        <div className="absolute inset-0 [clip-path:polygon(50%_0,100%_25%,100%_75%,50%_100%,0_75%,0_25%)] ring-2 ring-inset" style={{ boxShadow: `inset 0 0 0 2px ${phaseTone.stroke}` }} />
      )}
    </button>
  );
}

interface MatrixProps {
  pushToast: (t: any) => void;
  gamifyEnabled?: boolean;
}

export function Matrix({ pushToast, gamifyEnabled = false }: MatrixProps) {
  const embedded = useIsEmbeddedSurface();
  const [intentions, setIntentions] = useState<RoutingIntention[]>([]);
  const [sel, setSel] = useState<string | undefined>(undefined);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const cells = await invoke<RoutingIntention[]>('get_routing_intentions');
      setIntentions(Array.isArray(cells) ? cells : []);
      setSel(prev => (prev && cells.some(c => c.id === prev) ? prev : cells[0]?.id));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Routing policies load failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => {
    refresh();
    // Embedded mini-render: one initial fetch only, no repeating poll.
    if (embedded) return;
    const id = setInterval(refresh, MATRIX_POLL_MS);
    return () => clearInterval(id);
  }, [refresh, embedded]);

  const nudge = useCallback(async (axis: RoutingIntention, direction: 'promote' | 'doubt') => {
    setBusy(true);
    try {
      await invoke('nudge_routing_intention', { axis: axis.id, direction });
      void recordGamifyGuiEvent(
        'palette_navigation',
        { axis: axis.id, direction, surface: 'matrix' },
        { enabled: gamifyEnabled },
      );
      pushToast({
        tone: direction === 'promote' ? 'ok' : 'warn',
        title: direction === 'promote' ? 'Axis promoted' : 'Axis doubted',
        body: `${axis.branch} routing weight ${direction === 'promote' ? 'increased' : 'reduced'}.`,
        cmd: `vox config routing ${axis.id} ${direction}`,
        cause: 'backend-ok',
      });
      await refresh();
    } catch (err) {
      pushToast({ tone: 'warn', title: `Routing ${direction} failed`, body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(false);
    }
  }, [pushToast, refresh, gamifyEnabled]);

  const active = intentions.find(i => i.id === sel) || intentions[0];

  if (loading) return (
    <div className="p-8 text-center">
      <Glass className="p-12 inline-block">
        <p className="font-display uppercase tracking-[0.2em] text-text-muted">Loading routing policies…</p>
      </Glass>
    </div>
  );

  if (!active) return (
    <div className="p-8 text-center">
        <Glass className="p-12 inline-block">
            <p className="font-display uppercase tracking-[0.2em] text-text-muted">No routing policies active</p>
        </Glass>
    </div>
  );

  const groups: Record<string, RoutingIntention[]> = {};
  intentions.forEach((i) => { (groups[i.parent] = groups[i.parent] || []).push(i); });

  return (
    <div className="grid grid-cols-12 gap-5 p-5">
      <Glass className="col-span-12 xl:col-span-8 p-5">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">{useLabel('mat-routing')}</h2>
            <p className="mt-0.5 text-[11px] text-text-muted">Live model-routing priority axes · weight = how strongly the orchestrator favors each axis</p>
          </div>
        </div>
        <div className="mt-5 space-y-6">
          {Object.entries(groups).map(([parent, items]) => (
            <div key={parent}>
              <div className="mb-2 flex items-center gap-2 font-mono text-[10px] uppercase tracking-[0.2em] text-text-muted">
                <span className="h-px flex-1 bg-linear-to-r from-white/10 to-transparent" />
                <span className="text-text-muted">{parent}</span>
                <span className="h-px flex-1 bg-linear-to-l from-white/10 to-transparent" />
              </div>
              <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
                {items.map(i => <HexCell key={i.id} intention={i} onSelect={setSel} selected={sel === i.id} />)}
              </div>
            </div>
          ))}
        </div>
      </Glass>

      <Glass className="col-span-12 xl:col-span-4 p-5">
        <div className="flex items-center justify-between">
          <h3 className="font-display text-[14px] font-semibold tracking-wide text-text-primary">{useLabel('mat-axis')}</h3>
          <Pill phase={active.phase} />
        </div>
        <div className="mt-3 rounded-xl border border-border-subtle bg-overlay-subtle p-4">
          <div className="font-mono text-[10px] uppercase tracking-[0.2em] text-text-muted">{active.parent}</div>
          <div className="mt-1 font-display text-[20px] font-semibold tracking-tight text-text-primary">{active.branch}</div>
          <div className="mt-2 text-[12px] leading-relaxed text-text-muted">{active.note}</div>
          <div className="mt-4">
            <div className="flex items-center justify-between text-[10px] uppercase tracking-[0.2em] text-text-muted">
              <span>Weight</span><span className="font-mono text-text-secondary">{Math.round(active.conf*100)}%</span>
            </div>
            <div
              className="mt-1.5 h-2 overflow-hidden rounded-full bg-overlay-subtle"
              role="progressbar"
              aria-valuenow={Math.round(active.conf * 100)}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-label={`${active.branch} routing weight`}
            >
              <div className="h-full rounded-full bg-linear-to-r from-violet-400 via-cyan-400 to-emerald-400" style={{ width: `${active.conf*100}%` }} />
            </div>
          </div>
          <div className="mt-4 flex gap-2">
            <button type="button" disabled={busy} onClick={() => nudge(active, 'promote')} className="flex-1 rounded-md border border-emerald-400/30 bg-emerald-400/10 px-3 py-2 font-display text-[11px] uppercase tracking-[0.18em] text-emerald-300 hover:bg-emerald-400/20 transition disabled:opacity-40">Promote</button>
            <button type="button" disabled={busy} onClick={() => nudge(active, 'doubt')}   className="flex-1 rounded-md border border-amber-400/30 bg-amber-400/10 px-3 py-2 font-display text-[11px] uppercase tracking-[0.18em] text-amber-300 hover:bg-amber-400/20 transition disabled:opacity-40">Doubt</button>
          </div>
        </div>
      </Glass>
    </div>
  );
}
