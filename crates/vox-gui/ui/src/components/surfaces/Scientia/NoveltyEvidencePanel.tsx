import React from 'react';
import type { NoveltyAssessment, NoveltyVerdictKind } from './noveltyApi';

/** Human label + tone class for each verdict kind. */
function verdictMeta(kind: NoveltyVerdictKind): { label: string; tone: string } {
  switch (kind) {
    case 'novel':
      return { label: 'Novel', tone: 'border-emerald-400/40 bg-emerald-400/10 text-emerald-200' };
    case 'possibly_novel':
      return { label: 'Possibly novel', tone: 'border-violet-400/40 bg-violet-400/10 text-violet-200' };
    case 'not_novel':
      return { label: 'Not novel', tone: 'border-rose-400/40 bg-rose-400/10 text-rose-200' };
    case 'contradicted':
      return { label: 'Contradicted', tone: 'border-orange-400/40 bg-orange-400/10 text-orange-200' };
    case 'insufficient_evidence':
    default:
      return { label: 'Insufficient evidence', tone: 'border-amber-400/40 bg-amber-400/10 text-amber-200' };
  }
}

function num(n: number | null | undefined, digits = 2): string {
  return n == null ? '—' : n.toFixed(digits);
}

function SignalCell({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-0.5 rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-2">
      <span className="font-mono text-[9px] uppercase tracking-wider text-text-muted">{label}</span>
      <span className="font-mono text-[12px] text-text-secondary">{value}</span>
    </div>
  );
}

/**
 * Pure/presentational novelty-evidence panel: verdict chip + signal grid +
 * prior-art list + conflicts. Takes its `assessment` in via props so it is unit
 * testable without any Tauri boundary; the data fetch lives in DiscoveryReview.
 */
export function NoveltyEvidencePanel({ assessment }: { assessment: NoveltyAssessment }) {
  const { label, tone } = verdictMeta(assessment.verdict_kind);
  const insufficient = assessment.verdict_kind === 'insufficient_evidence';
  const s = assessment.signals;

  return (
    <div className="rounded-xl border border-border-subtle bg-overlay-subtle p-4">
      <div className="mb-3 flex items-center justify-between">
        <span className="font-display text-[10px] uppercase tracking-[0.2em] text-text-muted">
          Novelty Evidence
        </span>
        <span className={`rounded-full border px-3 py-0.5 font-mono text-[11px] ${tone}`}>{label}</span>
      </div>

      {insufficient && (
        <div className="mb-3 rounded-lg border border-amber-400/30 bg-amber-400/6 px-3 py-2 font-mono text-[11px] text-amber-200/90">
          <span aria-hidden="true">⚠</span> Retrieval failed or never ran — do not treat as novel.
          {assessment.insufficient_evidence_reason && (
            <div className="mt-1 text-amber-100/80">{assessment.insufficient_evidence_reason}</div>
          )}
        </div>
      )}

      {assessment.closest_hit_uri != null && (
        <div className="mb-3 font-mono text-[11px] text-text-muted">
          Closest prior art:{' '}
          <a
            href={assessment.closest_hit_uri}
            target="_blank"
            rel="noreferrer"
            className="text-brass underline decoration-dotted hover:text-brass/80"
          >
            {assessment.closest_hit_uri}
          </a>
          {assessment.closest_score != null && (
            <span className="text-text-muted"> · sim {num(assessment.closest_score)}</span>
          )}
        </div>
      )}

      {/* signal grid */}
      <div className="mb-4 grid grid-cols-2 gap-2 sm:grid-cols-3">
        <SignalCell label="Max semantic" value={num(s.max_semantic)} />
        <SignalCell label="Max lexical" value={num(s.max_lexical)} />
        <SignalCell label="Near-hit count" value={String(s.near_hit_count)} />
        <SignalCell label="Top-hit citations" value={s.top_hit_citations == null ? '—' : String(s.top_hit_citations)} />
        <SignalCell label="Sources succeeded" value={String(s.sources_succeeded)} />
        <SignalCell label="Excluded future hits" value={String(assessment.excluded_future_hits)} />
      </div>

      {/* prior art */}
      <div className="mb-1 font-mono text-[10px] uppercase tracking-wider text-text-muted">Closest prior art</div>
      {assessment.prior_art.length === 0 ? (
        <div className="rounded-lg border border-border-subtle px-3 py-2 font-mono text-[11px] text-text-muted">
          No prior-art hits.
        </div>
      ) : (
        <ul className="space-y-1.5">
          {assessment.prior_art.map((h, i) => (
            <li key={`${h.work_uri}-${i}`} className="rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-2">
              <a
                href={h.work_uri}
                target="_blank"
                rel="noreferrer"
                className="text-[12.5px] text-text-secondary underline decoration-dotted hover:text-brass"
              >
                {h.title || h.work_uri}
              </a>
              <div className="mt-1 flex flex-wrap items-center gap-x-3 font-mono text-[10px] text-text-muted">
                {h.year != null && <span>{h.year}</span>}
                {h.cited_by_count != null && <span>{h.cited_by_count} citations</span>}
                {h.semantic_score != null && <span className="text-brass">sim {num(h.semantic_score)}</span>}
              </div>
            </li>
          ))}
        </ul>
      )}

      {/* conflicts */}
      {assessment.conflicts.length > 0 && (
        <div className="mt-4">
          <div className="mb-1 font-mono text-[10px] uppercase tracking-wider text-rose-300/80">
            Evidence conflicts
          </div>
          <div className="space-y-2">
            {assessment.conflicts.map((c, i) => (
              <div key={i} className="rounded-lg border border-rose-400/20 bg-rose-400/4 px-3 py-2 font-mono text-[11px]">
                <div className="mb-1 text-rose-200/90">
                  conflict score {num(c.conflict_score)}
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <div className="text-emerald-300/80">Supporting ({c.supporting.length})</div>
                    {c.supporting.map((h, j) => (
                      <div key={j} className="truncate text-text-muted" title={h.excerpt ?? h.work_uri}>
                        {h.work_uri}
                      </div>
                    ))}
                  </div>
                  <div>
                    <div className="text-rose-300/80">Contradicting ({c.contradicting.length})</div>
                    {c.contradicting.map((h, j) => (
                      <div key={j} className="truncate text-text-muted" title={h.excerpt ?? h.work_uri}>
                        {h.work_uri}
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
