import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { PUBLICATION_STAGES, groupByStage, PublicationManifest } from '../../../lib/pipeline';

interface ExecuteOutput {
  exit_code: number;
  stdout: string;
  stderr: string;
}

interface ClaimRow {
  id?: number;
  text?: string;
  verdict_label?: string;
  confidence?: number;
}

interface ClaimsPayload {
  publication_id: string;
  claim_count: number;
  claims: ClaimRow[];
}

interface VenueRecommendation {
  candidate_class: string;
  recommended_venues: string[];
  reply_window_days: number;
  negative_result_quota: number;
  critic_allowed: boolean;
  atlas_gate_applies: boolean;
}

export function PublicationsView({ pushToast }: SurfaceDecoratorProps) {
  const [manifests, setManifests] = useState<PublicationManifest[]>([]);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<PublicationManifest | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setManifests(await invoke<PublicationManifest[]>('list_publication_manifests', { limit: 200 }));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Publications load failed', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => { refresh(); }, [refresh]);

  const groups = groupByStage(manifests);

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="font-display text-lg text-zinc-100 tracking-wider uppercase">Publication Pipeline</h2>
        <button onClick={refresh} disabled={loading}
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs hover:bg-white/[0.06]">
          {loading ? 'Loading…' : 'Refresh'}
        </button>
      </div>
      <div className="flex gap-3 overflow-x-auto pb-3 [scrollbar-width:thin] [scrollbar-color:rgba(255,255,255,0.15)_transparent]">
        {PUBLICATION_STAGES.map(stage => (
          <div key={stage} className="w-44 shrink-0">
            <div className="mb-2 flex items-center justify-between">
              <span className="font-mono text-[10px] uppercase tracking-wide text-zinc-400">{stage.replace(/_/g, ' ')}</span>
              <span className="rounded-full bg-white/[0.05] px-1.5 font-mono text-[9px] text-zinc-500">{groups[stage].length}</span>
            </div>
            <div className="space-y-2">
              {groups[stage].map(m => (
                <button
                  key={m.publication_id}
                  onClick={() => setSelected(m)}
                  className={`w-full rounded-lg border bg-white/[0.02] p-2 text-left transition-colors hover:bg-white/[0.05] ${
                    selected?.publication_id === m.publication_id ? 'border-cyan/50' : 'border-white/10'
                  }`}
                >
                  <div className="truncate font-mono text-[11px] text-zinc-200">{m.publication_id}</div>
                  <div className="text-[10px] text-zinc-500">{m.content_type}</div>
                </button>
              ))}
              {groups[stage].length === 0 && <div className="rounded-lg border border-dashed border-white/5 p-2 text-center text-[10px] text-zinc-600">—</div>}
            </div>
          </div>
        ))}
      </div>

      {selected && (
        <PublicationDetail
          manifest={selected}
          onClose={() => setSelected(null)}
          pushToast={pushToast}
        />
      )}
    </section>
  );
}

/**
 * Drill-down panel for one publication (F4): its lifecycle state, extracted
 * claims (`vox scientia claims`), and the venue-routing recommendation for its
 * class (`vox scientia publication-venue-recommend`). Both reuse the shared
 * `execute_command` bridge over existing CLI subcommands — no new backend.
 */
function PublicationDetail({
  manifest,
  onClose,
  pushToast,
}: {
  manifest: PublicationManifest;
  onClose: () => void;
  pushToast: SurfaceDecoratorProps['pushToast'];
}) {
  const [claims, setClaims] = useState<ClaimsPayload | null>(null);
  const [venue, setVenue] = useState<VenueRecommendation | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setBusy(true);
    setClaims(null);
    setVenue(null);
    try {
      const claimsOut = await invoke<ExecuteOutput>('execute_command', {
        path: ['scientia', 'claims'],
        args: { __argv: ['--publication-id', manifest.publication_id] },
      });
      if (claimsOut.exit_code === 0) {
        try {
          setClaims(JSON.parse(claimsOut.stdout) as ClaimsPayload);
        } catch (parseErr) {
          pushToast({ tone: 'warn', title: 'Claims (parse error)', body: `${String(parseErr)} — raw: ${claimsOut.stdout.slice(0, 200)}` });
        }
      } else {
        pushToast({ tone: 'warn', title: 'Claims', body: claimsOut.stderr || `exit ${claimsOut.exit_code}` });
      }

      const venueOut = await invoke<ExecuteOutput>('execute_command', {
        path: ['scientia', 'publication-venue-recommend'],
        args: { __argv: ['--candidate-class', manifest.content_type] },
      });
      if (venueOut.exit_code === 0) {
        try {
          setVenue(JSON.parse(venueOut.stdout) as VenueRecommendation);
        } catch (parseErr) {
          pushToast({ tone: 'warn', title: 'Venue (parse error)', body: `${String(parseErr)} — raw: ${venueOut.stdout.slice(0, 200)}` });
        }
      }
      // A non-zero venue exit is expected when content_type isn't a known
      // finding class; leave venue null and show the "no routing" state.
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Publication detail', body: String(err) });
    } finally {
      setBusy(false);
    }
  }, [manifest.publication_id, manifest.content_type, pushToast]);

  useEffect(() => { load(); }, [load]);

  // Lifecycle stepper: the ordered stages a publication passes through. The
  // current stage is derived from the manifest state via the same stage grouping
  // the board uses.
  const _stageMap = groupByStage([manifest]);
  const stageIdx = PUBLICATION_STAGES.findIndex(s => (_stageMap[s]?.length ?? 0) > 0);
  const resolvedStageIdx = stageIdx === -1 ? 0 : stageIdx;

  return (
    <div className="rounded-xl border border-cyan/30 bg-cyan/[0.02] p-4">
      <div className="mb-3 flex items-center justify-between">
        <div className="min-w-0">
          <div className="truncate font-mono text-sm text-zinc-100">{manifest.publication_id}</div>
          <div className="font-mono text-[11px] text-zinc-500">{manifest.content_type} · {manifest.state}</div>
        </div>
        <div className="flex items-center gap-2">
          <button onClick={load} disabled={busy}
            className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs hover:bg-white/[0.06]">
            {busy ? 'Loading…' : 'Reload'}
          </button>
          <button onClick={onClose}
            className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs hover:bg-white/[0.06]">
            Close
          </button>
        </div>
      </div>

      {/* Lifecycle stepper */}
      <div className="mb-4 flex flex-wrap items-center gap-1.5">
        {PUBLICATION_STAGES.map((s, i) => (
          <React.Fragment key={s}>
            <span className={`rounded px-2 py-0.5 font-mono text-[10px] uppercase tracking-wider ${
              i <= resolvedStageIdx ? 'bg-cyan/15 text-cyan' : 'bg-white/5 text-zinc-600'
            }`}>{s.replace(/_/g, ' ')}</span>
            {i < PUBLICATION_STAGES.length - 1 && <span className="text-zinc-600">›</span>}
          </React.Fragment>
        ))}
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        {/* Claims */}
        <div>
          <div className="mb-2 font-mono text-[10px] uppercase tracking-wider text-zinc-500">
            Claims {claims ? `(${claims.claim_count})` : ''}
          </div>
          {!claims || claims.claims.length === 0 ? (
            <div className="rounded-lg border border-white/5 bg-white/[0.02] p-3 text-[12px] text-zinc-500">
              {busy ? 'Loading…' : 'No extracted claims. Run claim extraction first.'}
            </div>
          ) : (
            <ul className="space-y-1">
              {claims.claims.map((c, i) => (
                <li key={c.id ?? i} className="rounded-lg border border-white/10 bg-white/[0.02] p-2">
                  <div className="text-[12px] text-zinc-200">{c.text ?? '(no text)'}</div>
                  {c.verdict_label && (
                    <div className="mt-0.5 font-mono text-[10px] text-zinc-500">
                      {c.verdict_label}{typeof c.confidence === 'number' ? ` · ${c.confidence.toFixed(2)}` : ''}
                    </div>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>

        {/* Venue routing */}
        <div>
          <div className="mb-2 font-mono text-[10px] uppercase tracking-wider text-zinc-500">Venue routing</div>
          {!venue ? (
            <div className="rounded-lg border border-white/5 bg-white/[0.02] p-3 text-[12px] text-zinc-500">
              {busy ? 'Loading…' : `No routing for class "${manifest.content_type}".`}
            </div>
          ) : (
            <div className="space-y-2 rounded-lg border border-white/10 bg-white/[0.02] p-3">
              <div className="flex flex-wrap gap-1.5">
                {venue.recommended_venues.length === 0 ? (
                  <span className="font-mono text-[11px] text-zinc-500">No recommended venues.</span>
                ) : venue.recommended_venues.map(v => (
                  <span key={v} className="rounded bg-cyan/10 px-2 py-0.5 font-mono text-[11px] text-cyan">{v}</span>
                ))}
              </div>
              <div className="grid grid-cols-2 gap-x-3 gap-y-1 font-mono text-[11px] text-zinc-400">
                <span>Reply window</span><span className="text-zinc-200">{venue.reply_window_days}d</span>
                <span>Neg-result quota</span><span className="text-zinc-200">{venue.negative_result_quota}</span>
                <span>Critic allowed</span><span className={venue.critic_allowed ? 'text-emerald-300' : 'text-amber-300'}>{venue.critic_allowed ? 'yes' : 'no'}</span>
                <span>Atlas gate</span><span className="text-zinc-200">{venue.atlas_gate_applies ? 'applies' : '—'}</span>
              </div>
              {!venue.critic_allowed && (
                <div className="rounded border border-amber-500/20 bg-amber-500/[0.04] p-2 text-[11px] text-amber-300/90">
                  Venue forbids LLM-critic approvals — add a second human approver.
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

