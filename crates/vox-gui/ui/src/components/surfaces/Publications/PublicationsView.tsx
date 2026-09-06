import React, { useCallback, useEffect, useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { invoke } from '@tauri-apps/api/core';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { PUBLICATION_STAGES, groupByStage, PublicationManifest } from '../../../lib/pipeline';
import { useLabel } from '../../../hooks/useLanguage';

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
      pushToast({ tone: 'warn', title: 'Publications load failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => { refresh(); }, [refresh]);

  const groups = groupByStage(manifests);

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="font-display text-lg text-text-primary tracking-wider uppercase">{useLabel('pub-pipeline')}</h2>
        <button onClick={refresh} disabled={loading}
          className="rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-1.5 text-xs hover:bg-overlay-subtle">
          {loading ? 'Loading…' : 'Refresh'}
        </button>
      </div>
      <div className="flex gap-3 overflow-x-auto pb-3 scrollbar-thin [scrollbar-color:rgba(255,255,255,0.15)_transparent]">
        {PUBLICATION_STAGES.map(stage => (
          <div key={stage} className="w-44 shrink-0">
            <div className="mb-2 flex items-center justify-between">
              <span className="font-mono text-[10px] uppercase tracking-wide text-text-muted">{stage.replace(/_/g, ' ')}</span>
              <span className="rounded-full bg-overlay-subtle px-1.5 font-mono text-[9px] text-text-muted">{groups[stage].length}</span>
            </div>
            <div className="space-y-2">
              {groups[stage].map(m => (
                <button
                  key={m.publication_id}
                  onClick={() => setSelected(m)}
                  className={`w-full rounded-lg border bg-overlay-subtle p-2 text-left transition-colors hover:bg-overlay-subtle ${
                    selected?.publication_id === m.publication_id ? 'border-cyan/50' : 'border-border-subtle'
                  }`}
                >
                  <div className="truncate font-mono text-[11px] text-text-secondary">{m.publication_id}</div>
                  <div className="text-[10px] text-text-muted">{m.content_type}</div>
                </button>
              ))}
              {groups[stage].length === 0 && <div className="rounded-lg border border-dashed border-border-subtle p-2 text-center text-[10px] text-text-muted">—</div>}
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
          pushToast({ tone: 'warn', title: 'Claims (parse error)', body: `${String(parseErr)} — raw: ${claimsOut.stdout.slice(0, 200)}`, cause: 'backend-error' });
        }
      } else {
        pushToast({ tone: 'warn', title: 'Claims', body: claimsOut.stderr || `exit ${claimsOut.exit_code}`, cause: 'backend-error' });
      }

      const venueOut = await invoke<ExecuteOutput>('execute_command', {
        path: ['scientia', 'publication-venue-recommend'],
        args: { __argv: ['--candidate-class', manifest.content_type] },
      });
      if (venueOut.exit_code === 0) {
        try {
          setVenue(JSON.parse(venueOut.stdout) as VenueRecommendation);
        } catch (parseErr) {
          pushToast({ tone: 'warn', title: 'Venue (parse error)', body: `${String(parseErr)} — raw: ${venueOut.stdout.slice(0, 200)}`, cause: 'backend-error' });
        }
      }
      // A non-zero venue exit is expected when content_type isn't a known
      // finding class; leave venue null and show the "no routing" state.
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Publication detail', body: sanitizeErrorForToast(err), cause: 'backend-error' });
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
          <div className="truncate font-mono text-sm text-text-primary">{manifest.publication_id}</div>
          <div className="font-mono text-[11px] text-text-muted">{manifest.content_type} · {manifest.state}</div>
        </div>
        <div className="flex items-center gap-2">
          <button onClick={load} disabled={busy}
            className="rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-1.5 text-xs hover:bg-overlay-subtle">
            {busy ? 'Loading…' : 'Reload'}
          </button>
          <button onClick={onClose}
            className="rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-1.5 text-xs hover:bg-overlay-subtle">
            Close
          </button>
        </div>
      </div>

      {/* Lifecycle stepper */}
      <div className="mb-4 flex flex-wrap items-center gap-1.5">
        {PUBLICATION_STAGES.map((s, i) => (
          <React.Fragment key={s}>
            <span className={`rounded px-2 py-0.5 font-mono text-[10px] uppercase tracking-wider ${
              i <= resolvedStageIdx ? 'bg-cyan/15 text-cyan' : 'bg-overlay-subtle text-text-muted'
            }`}>{s.replace(/_/g, ' ')}</span>
            {i < PUBLICATION_STAGES.length - 1 && <span className="text-text-muted">›</span>}
          </React.Fragment>
        ))}
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        {/* Claims */}
        <div>
          <div className="mb-2 font-mono text-[10px] uppercase tracking-wider text-text-muted">
            Claims {claims ? `(${claims.claim_count})` : ''}
          </div>
          {!claims || claims.claims.length === 0 ? (
            <div className="rounded-lg border border-border-subtle bg-overlay-subtle p-3 text-[12px] text-text-muted">
              {busy ? 'Loading…' : 'No extracted claims. Run claim extraction first.'}
            </div>
          ) : (
            <ul className="space-y-1">
              {claims.claims.map((c, i) => (
                <li key={c.id ?? i} className="rounded-lg border border-border-subtle bg-overlay-subtle p-2">
                  <div className="text-[12px] text-text-secondary">{c.text ?? '(no text)'}</div>
                  {c.verdict_label && (
                    <div className="mt-0.5 font-mono text-[10px] text-text-muted">
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
          <div className="mb-2 font-mono text-[10px] uppercase tracking-wider text-text-muted">Venue routing</div>
          {!venue ? (
            <div className="rounded-lg border border-border-subtle bg-overlay-subtle p-3 text-[12px] text-text-muted">
              {busy ? 'Loading…' : `No routing for class "${manifest.content_type}".`}
            </div>
          ) : (
            <div className="space-y-2 rounded-lg border border-border-subtle bg-overlay-subtle p-3">
              <div className="flex flex-wrap gap-1.5">
                {venue.recommended_venues.length === 0 ? (
                  <span className="font-mono text-[11px] text-text-muted">No recommended venues.</span>
                ) : venue.recommended_venues.map(v => (
                  <span key={v} className="rounded-sm bg-cyan/10 px-2 py-0.5 font-mono text-[11px] text-cyan">{v}</span>
                ))}
              </div>
              <div className="grid grid-cols-2 gap-x-3 gap-y-1 font-mono text-[11px] text-text-muted">
                <span>Reply window</span><span className="text-text-secondary">{venue.reply_window_days}d</span>
                <span>Neg-result quota</span><span className="text-text-secondary">{venue.negative_result_quota}</span>
                <span>Critic allowed</span><span className={venue.critic_allowed ? 'text-emerald-300' : 'text-amber-300'}>{venue.critic_allowed ? 'yes' : 'no'}</span>
                <span>Atlas gate</span><span className="text-text-secondary">{venue.atlas_gate_applies ? 'applies' : '—'}</span>
              </div>
              {!venue.critic_allowed && (
                <div className="rounded-sm border border-amber-500/20 bg-amber-500/4 p-2 text-[11px] text-amber-300/90">
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

