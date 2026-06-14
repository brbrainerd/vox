import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';

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
  Abstain: 'bg-zinc-500/15 text-zinc-300 ring-1 ring-zinc-500/30',
};

function VerdictBadge({ verdict }: { verdict: string | null }) {
  const label = verdict ?? 'pending';
  const cls = verdict ? VERDICT_STYLE[verdict] ?? VERDICT_STYLE.Abstain : 'bg-white/5 text-zinc-400 ring-1 ring-white/10';
  return (
    <span className={`rounded px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-wider ${cls}`}>{label}</span>
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
        pushToast({ tone: 'warn', title: 'Load claims failed', body: out.stderr || `exit ${out.exit_code}` });
        setClaims([]);
      } else {
        const parsed = JSON.parse(out.stdout) as { claims?: ClaimRow[] };
        setClaims(parsed.claims ?? []);
      }
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Load claims failed', body: String(err) });
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
      });
      if (out.exit_code === 0) {
        await loadClaims();
      }
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Extraction failed', body: String(err) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="space-y-4">
      <div>
        <h2 className="font-display text-lg tracking-wider text-zinc-100 uppercase">Scientia Claims</h2>
        <p className="font-mono text-xs text-zinc-500">Atomic claim extraction + verification ledger</p>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <input
          type="text"
          value={publicationId}
          onChange={(e) => setPublicationId(e.target.value)}
          placeholder="publication id"
          className="bg-void min-w-[16rem] flex-1 rounded-lg border border-white/10 bg-black/30 px-3 py-1.5 font-mono text-sm text-zinc-200 focus:border-cyan focus:outline-none"
        />
        <button
          type="button"
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs uppercase tracking-wider hover:bg-white/[0.06] disabled:opacity-40"
          disabled={busy || !publicationId.trim()}
          onClick={extract}
        >
          {busy ? 'Working…' : 'Extract claims'}
        </button>
        <button
          type="button"
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs uppercase tracking-wider hover:bg-white/[0.06] disabled:opacity-40"
          disabled={busy || !publicationId.trim()}
          onClick={loadClaims}
        >
          Load
        </button>
      </div>

      {claims === null && (
        <div className="font-mono text-xs text-zinc-500">
          Enter a publication id, then Extract (runs the pipeline) or Load (reads persisted claims).
        </div>
      )}
      {claims !== null && claims.length === 0 && (
        <div className="font-mono text-xs text-zinc-500">No claims recorded for this publication yet.</div>
      )}
      {claims !== null && claims.length > 0 && (
        <div className="space-y-2" role="list" aria-live="polite">
          <div className="font-mono text-[10px] uppercase tracking-wider text-zinc-500">
            {claims.length} claim{claims.length === 1 ? '' : 's'}
          </div>
          {claims.map((c) => (
            <div key={c.claim_id} role="listitem" className="rounded-xl border border-white/10 bg-white/[0.02] p-3">
              <div className="mb-1 flex items-center gap-2">
                <VerdictBadge verdict={c.verdict} />
                {c.confidence != null && (
                  <span className="font-mono text-[10px] text-zinc-400">conf {c.confidence.toFixed(2)}</span>
                )}
                {c.verifiability_score != null && (
                  <span className="font-mono text-[10px] text-zinc-500">vscore {c.verifiability_score.toFixed(2)}</span>
                )}
                {c.is_numeric && (
                  <span className="rounded bg-cyan/10 px-1 font-mono text-[9px] uppercase tracking-wider text-cyan">numeric</span>
                )}
                {c.verifier_model && (
                  <span className="ml-auto font-mono text-[9px] text-zinc-600">{c.verifier_model}</span>
                )}
              </div>
              <div className="text-sm text-zinc-200">{c.text}</div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
