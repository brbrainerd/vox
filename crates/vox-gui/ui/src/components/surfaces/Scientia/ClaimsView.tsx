import React, { useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { invoke } from '@tauri-apps/api/core';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { useLabel } from '../../../hooks/useLanguage';

interface ExecuteOutput {
  exit_code: number;
  stdout: string;
  stderr: string;
}

interface ClaimRow {
  claim_id: number;
  text: string;
  is_numeric: boolean;
  verifiability_score: number | null;
  verdict: string | null;
  confidence: number | null;
  verifier_model: string | null;
  created_at_ms: number;
}

const VERDICT_STYLE: Record<string, string> = {
  Supported: 'bg-emerald-500/15 text-emerald-300 ring-1 ring-emerald-500/30',
  Contested: 'bg-amber-500/15 text-amber-300 ring-1 ring-amber-500/30',
  Contradicted: 'bg-red-500/15 text-red-300 ring-1 ring-red-500/30',
  Abstain: 'bg-overlay-subtle text-text-secondary ring-1 ring-border-subtle/30',
};

export function VerdictBadge({ verdict }: { verdict: string | null }) {
  const label = verdict ?? 'pending';
  const cls = verdict ? VERDICT_STYLE[verdict] ?? VERDICT_STYLE.Abstain : 'bg-overlay-subtle text-text-muted ring-1 ring-white/10';
  return (
    <span className={`rounded-sm px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-wider ${cls}`}>{label}</span>
  );
}

/**
 * Scientia claims ledger: extract a publication's atomic claims (VeriScore →
 * atomic → span → MiniCheck) and view them with verdicts. Every command routes
 * through the shared `execute_command` path (the runAction seam).
 */
export function ClaimsView({ pushToast }: SurfaceDecoratorProps) {
  const [publicationId, setPublicationId] = useState('');
  const [claims, setClaims] = useState<ClaimRow[] | null>(null);
  const [busy, setBusy] = useState(false);

  const run = (path: string[]): Promise<ExecuteOutput> =>
    invoke<ExecuteOutput>('execute_command', { path, args: { publication_id: publicationId } });

  const loadClaims = async () => {
    if (!publicationId.trim()) return;
    setBusy(true);
    try {
      const out = await run(['scientia', 'claims']);
      if (out.exit_code !== 0) {
        pushToast({ tone: 'warn', title: 'Load claims failed', body: out.stderr || `exit ${out.exit_code}`, cause: 'backend-error' });
        setClaims([]);
      } else {
        const parsed = JSON.parse(out.stdout) as { claims?: ClaimRow[] };
        setClaims(parsed.claims ?? []);
      }
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Load claims failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      setClaims([]);
    } finally {
      setBusy(false);
    }
  };

  const extract = async () => {
    if (!publicationId.trim()) return;
    setBusy(true);
    try {
      const out = await run(['scientia', 'publication-extract-claims']);
      pushToast({
        tone: out.exit_code === 0 ? 'ok' : 'warn',
        title: 'Claim extraction',
        body: out.exit_code === 0 ? 'Extraction complete' : out.stderr || `exit ${out.exit_code}`,
        cause: out.exit_code === 0 ? 'backend-ok' : 'backend-error',
      });
      if (out.exit_code === 0) {
        await loadClaims();
      }
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Extraction failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="space-y-4">
      <div>
        <h2 className="font-display text-lg tracking-wider text-text-primary uppercase">{useLabel('sci-claims')}</h2>
        <p className="font-mono text-xs text-text-muted">Atomic claim extraction + verification ledger</p>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <input
          type="text"
          value={publicationId}
          onChange={(e) => setPublicationId(e.target.value)}
          placeholder="publication id"
          className="bg-bg-base min-w-[16rem] flex-1 rounded-lg border border-border-subtle bg-black/30 px-3 py-1.5 font-mono text-sm text-text-secondary focus:border-cyan focus:outline-hidden"
        />
        <button
          type="button"
          className="rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-1.5 text-xs uppercase tracking-wider hover:bg-overlay-subtle disabled:opacity-40"
          disabled={busy || !publicationId.trim()}
          onClick={extract}
        >
          {busy ? 'Working…' : 'Extract claims'}
        </button>
        <button
          type="button"
          className="rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-1.5 text-xs uppercase tracking-wider hover:bg-overlay-subtle disabled:opacity-40"
          disabled={busy || !publicationId.trim()}
          onClick={loadClaims}
        >
          Load
        </button>
      </div>

      {claims === null && (
        <div className="font-mono text-xs text-text-muted">
          Enter a publication id, then Extract (runs the pipeline) or Load (reads persisted claims).
        </div>
      )}
      {claims !== null && claims.length === 0 && (
        <div className="font-mono text-xs text-text-muted">No claims recorded for this publication yet.</div>
      )}
      {claims !== null && claims.length > 0 && (
        <div className="space-y-2" role="list" aria-live="polite">
          <div className="font-mono text-[10px] uppercase tracking-wider text-text-muted">
            {claims.length} claim{claims.length === 1 ? '' : 's'}
          </div>
          {claims.map((c) => (
            <div key={c.claim_id} role="listitem" className="rounded-xl border border-border-subtle bg-overlay-subtle p-3">
              <div className="mb-1 flex items-center gap-2">
                <VerdictBadge verdict={c.verdict} />
                {c.confidence != null && (
                  <span className="font-mono text-[10px] text-text-muted">conf {c.confidence.toFixed(2)}</span>
                )}
                {c.verifiability_score != null && (
                  <span className="font-mono text-[10px] text-text-muted">vscore {c.verifiability_score.toFixed(2)}</span>
                )}
                {c.is_numeric && (
                  <span className="rounded-sm bg-cyan/10 px-1 font-mono text-[9px] uppercase tracking-wider text-cyan">numeric</span>
                )}
                {c.verifier_model && (
                  <span className="ml-auto font-mono text-[9px] text-text-muted">{c.verifier_model}</span>
                )}
              </div>
              <div className="text-sm text-text-secondary">{c.text}</div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
