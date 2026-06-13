import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import {
  buildClaimReviewArgv,
  buildNanopubBuildArgv,
  extractTrustyUri,
  type ReviewDecision,
} from './discoveryReviewArgv';

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

/** Per-claim local state: the last human decision and any built Trusty URI. */
interface ReviewState {
  decision: ReviewDecision | null;
  trustyUri: string | null;
}

const DECISION_STYLE: Record<ReviewDecision, string> = {
  approve: 'bg-emerald-500/15 text-emerald-300 ring-1 ring-emerald-500/30',
  reject: 'bg-red-500/15 text-red-300 ring-1 ring-red-500/30',
  defer: 'bg-amber-500/15 text-amber-300 ring-1 ring-amber-500/30',
};

/**
 * Scientia DiscoveryReview (P2): human-gated claim review + nanopublication
 * build. Loads a publication's extracted claims (`vox scientia claims`), then
 * per claim records an Approve/Reject/Defer decision
 * (`publication-claim-review`) and, once approved, builds a signed
 * nanopublication (`publication-nanopub-build`) and surfaces its Trusty URI.
 *
 * Every mutation routes through the shared `execute_command` __argv bridge — no
 * new Tauri command. argv wiring lives in `discoveryReviewArgv.ts`.
 */
export function DiscoveryReviewView({ pushToast }: SurfaceDecoratorProps) {
  const [publicationId, setPublicationId] = useState('');
  const [orcid, setOrcid] = useState('');
  const [claims, setClaims] = useState<ClaimRow[] | null>(null);
  const [reviews, setReviews] = useState<Record<number, ReviewState>>({});
  const [reasons, setReasons] = useState<Record<number, string>>({});
  const [busy, setBusy] = useState(false);

  const pubId = publicationId.trim();

  const loadClaims = async () => {
    if (!pubId) return;
    setBusy(true);
    try {
      const out = await invoke<ExecuteOutput>('execute_command', {
        path: ['scientia', 'claims'],
        args: { __argv: ['--publication-id', pubId] },
      });
      if (out.exit_code !== 0) {
        pushToast({ tone: 'warn', title: 'Load claims failed', body: out.stderr || `exit ${out.exit_code}` });
        setClaims([]);
        return;
      }
      const parsed = JSON.parse(out.stdout) as { claims?: ClaimRow[] };
      setClaims(parsed.claims ?? []);
      setReviews({});
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Load claims failed', body: String(err) });
      setClaims([]);
    } finally {
      setBusy(false);
    }
  };

  const review = async (claimId: number, decision: ReviewDecision) => {
    if (!pubId) return;
    setBusy(true);
    try {
      const out = await invoke<ExecuteOutput>('execute_command', {
        path: ['scientia', 'publication-claim-review'],
        args: {
          __argv: buildClaimReviewArgv({ publicationId: pubId, claimId, decision, reason: reasons[claimId] }),
        },
      });
      if (out.exit_code !== 0) {
        pushToast({ tone: 'warn', title: 'Review failed', body: out.stderr || `exit ${out.exit_code}` });
        return;
      }
      setReviews((prev) => ({ ...prev, [claimId]: { decision, trustyUri: null } }));
      pushToast({ tone: 'ok', title: 'Claim review', body: `claim ${claimId} → ${decision}` });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Review failed', body: String(err) });
    } finally {
      setBusy(false);
    }
  };

  const buildNanopub = async (claimId: number) => {
    if (!pubId) return;
    setBusy(true);
    try {
      const out = await invoke<ExecuteOutput>('execute_command', {
        path: ['scientia', 'publication-nanopub-build'],
        args: { __argv: buildNanopubBuildArgv({ publicationId: pubId, claimId, orcid }) },
      });
      if (out.exit_code !== 0) {
        pushToast({ tone: 'warn', title: 'Nanopub build failed', body: out.stderr || `exit ${out.exit_code}` });
        return;
      }
      const uri = extractTrustyUri(out.stdout);
      setReviews((prev) => ({
        ...prev,
        [claimId]: { decision: prev[claimId]?.decision ?? 'approve', trustyUri: uri },
      }));
      pushToast({ tone: 'ok', title: 'Nanopublication built', body: uri });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Nanopub build failed', body: String(err) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="space-y-4">
      <div>
        <h2 className="font-display text-lg tracking-wider text-zinc-100 uppercase">Discovery Review</h2>
        <p className="font-mono text-xs text-zinc-500">
          Human-gated claim review → signed nanopublication build
        </p>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <input
          type="text"
          value={publicationId}
          onChange={(e) => setPublicationId(e.target.value)}
          placeholder="publication id"
          className="bg-void min-w-[16rem] flex-1 rounded-lg border border-white/10 bg-black/30 px-3 py-1.5 font-mono text-sm text-zinc-200 focus:border-cyan focus:outline-none"
        />
        <input
          type="text"
          value={orcid}
          onChange={(e) => setOrcid(e.target.value)}
          placeholder="orcid (optional)"
          className="bg-void min-w-[16rem] flex-1 rounded-lg border border-white/10 bg-black/30 px-3 py-1.5 font-mono text-sm text-zinc-200 focus:border-cyan focus:outline-none"
        />
        <button
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs uppercase tracking-wider hover:bg-white/[0.06] disabled:opacity-40"
          disabled={busy || !pubId}
          onClick={loadClaims}
        >
          {busy ? 'Working…' : 'Load claims'}
        </button>
      </div>

      {claims === null && (
        <div className="font-mono text-xs text-zinc-500">
          Enter a publication id, then Load claims to review them.
        </div>
      )}
      {claims !== null && claims.length === 0 && (
        <div className="font-mono text-xs text-zinc-500">No claims recorded for this publication yet.</div>
      )}
      {claims !== null && claims.length > 0 && (
        <div className="space-y-2">
          <div className="font-mono text-[10px] uppercase tracking-wider text-zinc-500">
            {claims.length} claim{claims.length === 1 ? '' : 's'}
          </div>
          {claims.map((c) => {
            const state = reviews[c.claim_id];
            const approved = state?.decision === 'approve';
            return (
              <div key={c.claim_id} className="rounded-xl border border-white/10 bg-white/[0.02] p-3">
                <div className="mb-2 flex items-center gap-2">
                  <span className="font-mono text-[10px] text-zinc-500">#{c.claim_id}</span>
                  {state?.decision && (
                    <span
                      className={`rounded px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-wider ${DECISION_STYLE[state.decision]}`}
                    >
                      {state.decision}
                    </span>
                  )}
                </div>
                <div className="mb-2 text-sm text-zinc-200">{c.text}</div>

                <input
                  type="text"
                  value={reasons[c.claim_id] ?? ''}
                  onChange={(e) => setReasons((prev) => ({ ...prev, [c.claim_id]: e.target.value }))}
                  placeholder="reason (optional)"
                  className="mb-2 w-full rounded-lg border border-white/10 bg-black/30 px-2 py-1 font-mono text-xs text-zinc-200 focus:border-cyan focus:outline-none"
                />

                <div className="flex flex-wrap items-center gap-2">
                  <button
                    className="rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-2.5 py-1 text-xs uppercase tracking-wider text-emerald-300 hover:bg-emerald-500/20 disabled:opacity-40"
                    disabled={busy}
                    onClick={() => review(c.claim_id, 'approve')}
                  >
                    Approve
                  </button>
                  <button
                    className="rounded-lg border border-red-500/30 bg-red-500/10 px-2.5 py-1 text-xs uppercase tracking-wider text-red-300 hover:bg-red-500/20 disabled:opacity-40"
                    disabled={busy}
                    onClick={() => review(c.claim_id, 'reject')}
                  >
                    Reject
                  </button>
                  <button
                    className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-2.5 py-1 text-xs uppercase tracking-wider text-amber-300 hover:bg-amber-500/20 disabled:opacity-40"
                    disabled={busy}
                    onClick={() => review(c.claim_id, 'defer')}
                  >
                    Defer
                  </button>
                  <button
                    className="ml-auto rounded-lg border border-cyan/30 bg-cyan/10 px-2.5 py-1 text-xs uppercase tracking-wider text-cyan hover:bg-cyan/20 disabled:opacity-40"
                    disabled={busy || !approved}
                    title={approved ? 'Build a signed nanopublication' : 'Approve this claim first'}
                    onClick={() => buildNanopub(c.claim_id)}
                  >
                    Build nanopub
                  </button>
                </div>

                {state?.trustyUri && (
                  <div className="mt-2 break-all rounded-lg border border-cyan/20 bg-cyan/5 px-2 py-1 font-mono text-[10px] text-cyan">
                    {state.trustyUri}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}
