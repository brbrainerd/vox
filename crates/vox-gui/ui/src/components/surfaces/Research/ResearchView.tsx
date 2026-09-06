import React, { useCallback, useEffect, useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { invoke } from '@tauri-apps/api/core';
import { listenScientiaQueue } from '../../../transport';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { useLabel } from '../../../hooks/useLanguage';
import { PipelineTimeline } from '../../PipelineTimeline';
import { RESEARCH_STAGES, deriveStages } from '../../../lib/pipeline';
import { startResearchAsync } from './researchActions';
import { useIsEmbeddedSurface } from '../../dashboard/EmbeddedSurfaceContext';
import { ResearchClaimAccordion, type ResearchClaimRow } from './ResearchClaimAccordion';
import { HeadlineVerdictBanner } from './HeadlineVerdictBanner';

interface ResearchSession { id: number; status: string; query_text: string; started_at_ms: number; finished_at_ms: number | null; }

/** Raw claim shape as emitted by `get_research_session_detail` (snake_case,
 * matching `ResearchClaimDto` in `crates/vox-gui/src/commands/scientia.rs`). */
interface ResearchDetailClaim {
  claim_id: string;
  text: string;
  verdict: string;
  confidence: number;
  resample_stability: number;
  citation_urls: string[];
  corroboration_count: number;
}

// confidence_tier / claims / source_count / citation_precision are parsed
// out of the session's `artifact_json` on the Rust side (see
// `extract_research_summary` in scientia.rs) and are `undefined`/empty only
// when no artifact was persisted for the session (or it failed to parse) —
// the trust UI below stays hidden in that case rather than fabricating data.
interface ResearchDetail {
  session: ResearchSession;
  report_markdown: string | null;
  artifact_json: string | null;
  confidence_tier?: 'Direct' | 'Light' | 'DeepResearch';
  claims?: ResearchDetailClaim[];
  source_count?: number;
  citation_precision?: number;
}

/**
 * Map the backend's flat claim DTO onto `ResearchClaimAccordion`'s
 * `ResearchClaimRow` shape. Trust is derived from `corroboration_count`,
 * the pipeline's real distinct-domain independent-source count (see
 * `vox_search::corroboration` / `compute_corroboration_counts` in
 * `vox-research-shim`'s pipeline.rs) — a claim backed by 2+ distinct-domain
 * supporting citations reads as corroborated, 0 or 1 as uncorroborated.
 */
function toClaimRows(claims: ResearchDetailClaim[] | undefined): ResearchClaimRow[] {
  if (!claims) return [];
  return claims.map((c) => {
    const distinctUrls = Array.from(new Set(c.citation_urls));
    return {
      claimId: c.claim_id,
      text: c.text,
      verdict: c.verdict,
      confidence: c.confidence,
      resampleStability: c.resample_stability,
      citations: distinctUrls.map((url) => ({
        url,
        trust:
          c.corroboration_count >= 2
            ? { kind: 'corroborated' as const, sourceCount: c.corroboration_count }
            : { kind: 'uncorroborated' as const },
      })),
    };
  });
}

export function ResearchView({ pushToast }: SurfaceDecoratorProps) {
  const embedded = useIsEmbeddedSurface();
  const [query, setQuery] = useState('');
  const [running, setRunning] = useState(false);
  const [activeSessionId, setActiveSessionId] = useState<number | null>(null);
  const [sessions, setSessions] = useState<ResearchSession[]>([]);
  const [detail, setDetail] = useState<ResearchDetail | null>(null);

  const loadHistory = useCallback(async () => {
    try {
      setSessions(await invoke<ResearchSession[]>('list_research_sessions', { limit: 25 }));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'History load failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  }, [pushToast]);

  const openDetail = useCallback(async (id: number) => {
    try {
      setDetail(await invoke<ResearchDetail>('get_research_session_detail', { sessionId: id }));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Session load failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  }, [pushToast]);

  useEffect(() => { loadHistory(); }, [loadHistory]);

  // A2: refetch history whenever the persistent daemon's Scientia-queue watcher
  // signals a research-session transition; a 10 s interval is the fallback
  // (e.g. outside Tauri, where listen() rejects), mirroring ScientiaDashboard.
  useEffect(() => {
    // Embedded mini-render: the initial loadHistory() (separate effect above)
    // already populated the thumbnail; skip the repeating poll + subscription.
    if (embedded) return;
    const id = setInterval(loadHistory, 10_000);
    let unlisten: (() => void) | undefined;
    listenScientiaQueue(() => { void loadHistory(); })
      .then((fn) => { unlisten = fn; })
      .catch(() => { /* not in Tauri — interval fallback covers it */ });
    return () => { clearInterval(id); unlisten?.(); };
  }, [loadHistory, embedded]);

  // Once the active run reaches a terminal status, stop the running indicator
  // and open its detail so the answer (report_markdown ?? artifact_json) shows.
  useEffect(() => {
    if (activeSessionId == null) return;
    const s = sessions.find((x) => x.id === activeSessionId);
    if (s && (s.status === 'completed' || s.status === 'failed')) {
      setRunning(false);
      void openDetail(activeSessionId);
      setActiveSessionId(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessions, activeSessionId]);

  const run = async () => {
    if (!query.trim()) return;
    setRunning(true);
    setActiveSessionId(null);
    try {
      // A2: fire-and-forget via the persistent daemon's async executor. Returns
      // {session_id, task_id, status: "running"} immediately — does NOT block on
      // the pipeline. Status transitions arrive through the queue watcher below.
      const handle = await startResearchAsync({ query });
      setActiveSessionId(handle.session_id);
      await loadHistory();
    } catch (err) {
      setRunning(false);
      pushToast({ tone: 'warn', title: 'Research run failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  };

  return (
    <section className="space-y-4">
      <h2 className="font-display text-lg text-text-primary tracking-wider uppercase">{useLabel('research')}</h2>

      <div className="flex gap-2">
        <input value={query} onChange={e => setQuery(e.target.value)} placeholder="Ask a research question…"
          aria-label="Research question"
          onKeyDown={e => { if (e.key === 'Enter') void run(); }}
          className="flex-1 rounded-lg border border-border-subtle bg-black/40 px-3 py-2 text-sm text-text-secondary outline-hidden focus:border-brass/40" />
        <button type="button" onClick={run} disabled={running}
          className="rounded-lg border border-brass/30 bg-brass/10 px-4 py-2 text-sm text-brass hover:bg-brass/20 disabled:opacity-50">
          {running ? 'Running…' : 'Run'}
        </button>
      </div>
      {running && (
        <div className="rounded-lg border border-border-subtle bg-overlay-subtle p-3" aria-live="polite">
          <PipelineTimeline stages={RESEARCH_STAGES} statuses={deriveStages('active')} />
          <div className="mt-2 text-[11px] text-text-muted">
            Running in the background{activeSessionId != null ? ` (session ${activeSessionId})` : ''} — the answer opens automatically when it completes.
          </div>
        </div>
      )}

      <div>
        <div className="mb-2 flex items-center justify-between">
          <span className="font-display text-[12px] uppercase tracking-wide text-text-muted">Recent sessions</span>
          <button type="button" onClick={loadHistory} className="text-[11px] text-text-muted hover:text-text-secondary">Refresh</button>
        </div>
        {sessions.length === 0 ? (
          <div className="rounded-lg border border-dashed border-border-subtle py-6 text-center text-[11px] text-text-muted">
            No research sessions yet — ask a question above to run one.
          </div>
        ) : (
          <ul className="space-y-1">
            {sessions.map(s => (
              <li key={s.id}>
                <button type="button" onClick={() => openDetail(s.id)}
                  className="flex w-full items-center justify-between rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-2 text-left hover:bg-overlay-subtle">
                  <span className="truncate text-[12px] text-text-secondary">{s.query_text}</span>
                  <span className="ml-3 shrink-0 font-mono text-[10px] text-text-muted">{s.status}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {detail && (
        <div className="rounded-lg border border-border-subtle bg-overlay-subtle p-3">
          <div className="mb-2 flex items-center justify-between">
            <span className="text-[12px] text-text-secondary">Session {detail.session.id}</span>
            <button type="button" onClick={() => setDetail(null)} className="text-[11px] text-text-muted hover:text-text-secondary">Close</button>
          </div>
          <PipelineTimeline stages={RESEARCH_STAGES} statuses={deriveStages(detail.session.status)} />
          {(() => {
            const claimRows = toClaimRows(detail.claims);
            // Count citations the banner can actually back up as
            // corroborated, not the raw total source count — otherwise the
            // headline number contradicts the per-citation TrustChips shown
            // in the accordion below when few/no citations are corroborated.
            const corroboratingCitationCount = claimRows.reduce(
              (sum, c) => sum + c.citations.filter((cite) => cite.trust.kind === 'corroborated').length,
              0,
            );
            return (
              <>
                {claimRows.length > 0 && (
                  <HeadlineVerdictBanner
                    confidenceTier={detail.confidence_tier ?? 'DeepResearch'}
                    corroboratingSources={corroboratingCitationCount}
                    contestedClaims={claimRows.filter((c) => c.verdict === 'Contested').length}
                    totalClaims={claimRows.length}
                  />
                )}
                <pre className="mt-2 max-h-[360px] overflow-auto whitespace-pre-wrap text-[12px] text-text-secondary">
                  {detail.report_markdown ?? detail.artifact_json ?? '(no artifact persisted)'}
                </pre>
                {claimRows.length > 0 && (
                  <div className="mt-2">
                    <ResearchClaimAccordion claims={claimRows} sourceCount={detail.source_count ?? 0} />
                  </div>
                )}
              </>
            );
          })()}
        </div>
      )}
    </section>
  );
}
